#![feature(const_fn)]
#![feature(core_panic_info)]
#![feature(lang_items)]
#![no_std]

extern crate shared;

mod paging;
mod terminal;
mod vga;

use core::cell;
use core::cmp;
use core::fmt::write;
use core::mem::size_of;
use core::ops::DerefMut;
use core::panic;
use core::ptr;
use core::slice::from_raw_parts;
use core::str::from_utf8;
use shared::elf;
use shared::handoff;
use shared::memory;
use shared::multiboot;

extern "C" {
    fn kernel_handoff(
        mbinfo_addr: *const u64,
        page_table_addr: *const u32,
        kernel_entry_addr: *const u64,
        boot_info: *const handoff::BootInfo,
    ) -> !;
}

static mut TERMBUF: cell::RefCell<terminal::Buffer> = cell::RefCell::new(terminal::Buffer::new());

fn log_terminal(s: &str) {
    // Currently only one thread exists, so this is safe.
    unsafe {
        let mut termbuf = TERMBUF.borrow_mut();
        termbuf.write_line(s);
        vga::display_terminal(termbuf.deref_mut());
    }
}

struct TermWriter {
    buffer: [u8; 80],
    ndx: usize,
}

impl TermWriter {
    fn new() -> TermWriter {
        TermWriter {
            buffer: [0; 80],
            ndx: 0,
        }
    }

    fn flush(&mut self) {
        match from_utf8(&self.buffer[0..self.ndx]) {
            Ok(s) => log_terminal(s),
            Err(_) => (),
        }
        self.ndx = 0;
    }

    fn write_byte(&mut self, c: u8) {
        self.buffer[self.ndx] = c;
        self.ndx += 1;
        if self.ndx == 80 {
            self.flush();
        }
    }
}

impl core::fmt::Write for TermWriter {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        for c in s.bytes() {
            self.write_byte(c);
        }
        Ok(())
    }
}

fn write_terminal(args: core::fmt::Arguments) {
    let mut term_writer = TermWriter::new();
    let _ = write(&mut term_writer, args);
    term_writer.flush();
}

#[panic_handler]
#[no_mangle]
pub extern "C" fn panic_fmt(_info: &panic::PanicInfo) -> ! {
    /*
        let mut term_writer = TermWriter::new();
        let _ = write(&mut term_writer, format_args!("Panic in {} at line {}: ", file, line));
        let _ = write(&mut term_writer, panic_args);
        term_writer.flush();
    */
    loop {}
}

#[repr(C, packed)]
struct ModuleRaw {
    start: u32,
    end: u32,
    string: u32,
    reserved: u32,
}

struct Module {
    data: &'static [u8],
}

impl Module {
    fn from_raw(mr: &ModuleRaw) -> Self {
        let startp = mr.start as *const u8;
        let len = (mr.end - mr.start) as usize;
        Module {
            data: unsafe { from_raw_parts(startp, len) },
        }
    }
}

unsafe fn ptr_copy<T: Copy>(dst: *mut T, src: *const T, bytes: usize) {
    for i in 0..bytes {
        ptr::write(dst.offset(i as isize), ptr::read(src.offset(i as isize)));
    }
}

unsafe fn zero_frame(frame: *mut u8) {
    for i in 0..memory::PAGE_SIZE {
        *frame.offset(i as isize) = 0;
    }
}

// T must be repr(C, packed)
fn read_from_buffer<T>(buf: &[u8], off: usize) -> &T {
    let sz = size_of::<T>();
    assert!(sz + off <= buf.len());
    let ptr = unsafe { buf.as_ptr().offset(off as isize) };
    unsafe { &*(ptr as *const T) }
}

fn get_loader_extent(mbinfo: &multiboot::Info) -> (u64, u64) {
    let symtab_info = multiboot::get_section_header_table_info(mbinfo);
    let bounds = (0..symtab_info.entry_count)
        .map(|ndx| unsafe {
            elf::get_section_header_32(symtab_info.addr, symtab_info.entry_size, ndx)
        })
        .map(|header| (header.addr as u64, (header.addr + header.size) as u64))
        .filter(|&(lower, upper)| upper - lower > 0)
        .fold((u64::max_value(), 0), |(a, b), (c, d)| {
            (cmp::min(a, c), cmp::max(b, d))
        });

    (bounds.0, bounds.1 - bounds.0)
}

fn map_kernel(
    kernel: &[u8],
    addr_space: &mut paging::AddrSpace,
    alloc: &mut memory::FrameAllocator,
) {
    // Check alignment.
    assert!(kernel.as_ptr() as u64 % memory::PAGE_SIZE as u64 == 0);

    let elf_header: &elf::ElfHeaderRaw = read_from_buffer(kernel, 0);

    // Check that the kernel image is what we expect.
    assert!(
        elf_header.ident[0] == 0x7f
            && elf_header.ident[1] == 'E' as u8
            && elf_header.ident[2] == 'L' as u8
            && elf_header.ident[3] == 'F' as u8
    );
    assert!(elf_header.typ == elf::ElfType::Exec as u16);
    assert!(elf_header.machine == 62);

    // Map segments.
    for i in 0..(elf_header.phnum as usize) {
        let seg_offset = i * (elf_header.phentsize as usize) + (elf_header.phoff as usize);
        let seg_header: &elf::ProgramHeaderRaw = read_from_buffer(kernel, seg_offset);

        let segment_base = seg_header.offset as usize & !(memory::PAGE_SIZE - 1);
        let first_page = seg_header.vaddr / memory::PAGE_SIZE as u64;
        let last_page = (seg_header.vaddr + seg_header.memsz) / memory::PAGE_SIZE as u64;
        let num_pages = last_page + 1 - first_page;

        for pndx in 0..num_pages {
            let page = paging::Page(pndx + first_page);
            let copy_offset = segment_base + pndx as usize * memory::PAGE_SIZE;
            let frame_addr = alloc.get_frame() as u64;
            unsafe {
                zero_frame(frame_addr as *mut u8);
            }
            if copy_offset < kernel.len() {
                unsafe {
                    ptr_copy(
                        frame_addr as *mut u8,
                        kernel.as_ptr().offset(copy_offset as isize),
                        memory::PAGE_SIZE,
                    );
                }
            }
            let frame = paging::Frame(frame_addr / memory::PAGE_SIZE as u64);
            addr_space.map_to(page, frame, 0b1000, alloc);
        }
    }
}

fn map_identity(
    first_page: u64,
    page_count: u64,
    addr_space: &mut paging::AddrSpace,
    alloc: &mut memory::FrameAllocator,
) {
    for i in first_page..first_page + page_count {
        addr_space.map_to(paging::Page(i), paging::Frame(i), 0b1000, alloc);
    }
}

#[no_mangle]
pub extern "C" fn loader_entry(mbinfop: *const multiboot::Info) {
    let mbinfo = unsafe { &*mbinfop };
    let mod_raw_entries = unsafe {
        from_raw_parts(
            mbinfo.mods_addr as *const ModuleRaw,
            mbinfo.mods_count as usize,
        )
    };
    let mut mod_entries = mod_raw_entries.into_iter().map(Module::from_raw);
    // Kernel should be first (and only) module.
    let kernel_mod = mod_entries.next().expect("Kernel module not loaded.");
    let elf_header: &elf::ElfHeaderRaw = read_from_buffer(kernel_mod.data, 0);

    // Set up memory map and allocator.
    let loader_extent = get_loader_extent(mbinfo);
    let mut mem_map = memory::MemoryMap::from_multiboot(mbinfo);
    write_terminal(format_args!("{:x} {:x}", loader_extent.0, loader_extent.1));

    let mut mem_map_for_kernel = mem_map.clone();
    mem_map.reserve(
        kernel_mod.data.as_ptr() as u64,
        kernel_mod.data.len() as u64,
    );
    mem_map.reserve(loader_extent.0, loader_extent.1);
    mem_map.reserve(0, 0x100000);
    mem_map.reserve(mbinfop as u64, size_of::<multiboot::Info>() as u64);
    for i in 0..mem_map.num_entries as usize {
        let base = mem_map.entries[i].base;
        let length = mem_map.entries[i].length;
        write_terminal(format_args!("{:x} {:x}", base, length));
    }
    let mut alloc = memory::FrameAllocator::new(&mem_map);

    // Track region of memory used for kernel and paging tables.
    let kernel_begin = alloc.next_frame();

    // Set up paging for kernel.
    let mut addr_space = paging::AddrSpace::new(&mut alloc);
    map_kernel(kernel_mod.data, &mut addr_space, &mut alloc);
    // Identity map first 16 MiB
    map_identity(0, 4096, &mut addr_space, &mut alloc);

    let kernel_end = alloc.next_frame();
    mem_map_for_kernel.reserve(kernel_begin as u64, (kernel_end - kernel_begin) as u64);

    // Switch to 64 bit and call kernel.
    let mbinfo_addr = mbinfop as u64;
    let page_table_addr = addr_space.get_p4_addr() as u32;
    let kernel_entry_addr = elf_header.entry;

    let boot_info = handoff::BootInfo {
        mem_map: mem_map_for_kernel,
    };

    write_terminal(format_args!("{:x}", kernel_entry_addr));

    unsafe {
        kernel_handoff(
            &mbinfo_addr as *const u64,
            &page_table_addr as *const u32,
            &kernel_entry_addr as *const u64,
            &boot_info as *const _,
        );
    }
}
