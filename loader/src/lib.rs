#![feature(const_fn)]
#![feature(const_refcell_new)]
#![feature(lang_items)]
#![no_std]

extern crate rlibc;
extern crate shared;

mod paging;
mod terminal;
mod vga;

use core::cell;
use core::cmp;
use core::fmt::write;
use core::mem::size_of;
use core::ops::DerefMut;
use core::slice::from_raw_parts;
use core::str::from_utf8;
use shared::elf;
use shared::memory;
use shared::multiboot;

extern {
    fn kernel_handoff(mbinfo_addr: *const u64, page_table_addr: *const u64, kernel_entry_addr: *const u64) -> !;
}

static mut TERMBUF: cell::RefCell<terminal::Buffer> = cell::RefCell::new(terminal::Buffer::new());

fn log_terminal(s: &str)
{
    // Currently only one thread exists, so this is safe.
    unsafe {
        let mut termbuf = TERMBUF.borrow_mut();
        termbuf.write_line(s);
        vga::display_terminal(termbuf.deref_mut());
    }
}

struct BufWriter {
    buffer: [u8; 80],
    ndx: usize,
}

impl BufWriter {
    fn new() -> BufWriter {
        BufWriter {
            buffer: [0; 80],
            ndx: 0,
        }
    }
}

impl core::fmt::Write for BufWriter {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        let trunc = s.bytes().take(80-self.ndx);
        for c in trunc {
            self.buffer[self.ndx] = c;
            self.ndx += 1;
        }
        Ok(())
    }
}

fn write_terminal(args: core::fmt::Arguments) {
    let mut buf_writer = BufWriter::new();
    write(&mut buf_writer, args);
    buf_writer.buffer[79] = 0;

    match from_utf8(&buf_writer.buffer) {
        Ok(s) => log_terminal(s),
        Err(_) => panic!(),
    }
}

#[lang="panic_fmt"]
#[no_mangle]
pub extern fn panic_fmt(panic_args: ::core::fmt::Arguments, file: &'static str, line: u32) -> ! {
    let mut buf_writer = BufWriter::new();
    write(&mut buf_writer, format_args!("Panic in {} at line {}: ", file, line));
    write(&mut buf_writer, panic_args);
    buf_writer.buffer[79] = 0;

    match from_utf8(&buf_writer.buffer) {
        Ok(s) => log_terminal(s),
        Err(_) => (), // We're already panicking, there's nothing else to do.
    }

    loop { }
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
        .map(|ndx| unsafe { elf::get_section_header_32(symtab_info.addr, symtab_info.entry_size, ndx) })
        .map(|header| (header.addr as u64, (header.addr + header.size) as u64))
        .filter(|&(lower, upper)| upper - lower > 0)
        .fold((u64::max_value(), 0), |(a, b), (c, d)| (cmp::min(a, c), cmp::max(b, d)));

    (bounds.0, bounds.1 - bounds.0)
}

fn map_kernel(kernel: &[u8],
              addr_space: &mut paging::AddrSpace,
              alloc: &mut memory::FrameAllocator) {
    // Check alignment.
    assert!(kernel.as_ptr() as u64 % memory::PAGE_SIZE as u64 == 0);

    let elf_header: &elf::ElfHeaderRaw = read_from_buffer(kernel, 0);

    // Check that the kernel image is what we expect.
    assert!(elf_header.ident[0] == 0x7f
            && elf_header.ident[1] == 'E' as u8
            && elf_header.ident[2] == 'L' as u8
            && elf_header.ident[3] == 'F' as u8);
    assert!(elf_header.typ == elf::ElfType::Exec as u16);
    assert!(elf_header.machine == 62);

    // Map segments.
    for i in 0..(elf_header.phnum as usize) {
        let seg_offset = i * (elf_header.phentsize as usize) + (elf_header.phoff as usize);
        let seg_header: &elf::ProgramHeaderRaw = read_from_buffer(kernel, seg_offset);

        let first_frame = ((kernel.as_ptr() as u64) + seg_header.offset) / memory::PAGE_SIZE as u64;
        let first_page = seg_header.vaddr / memory::PAGE_SIZE as u64;
        let last_page = (seg_header.vaddr + seg_header.memsz) / memory::PAGE_SIZE as u64;
        let num_pages = last_page + 1 - first_page;

        for pndx in 0..num_pages {
            let page = paging::Page(pndx + first_page);
            let frame = paging::Frame(pndx + first_frame);
            addr_space.map_to(page, frame, 0b1000, alloc);
        }
    }
}

fn map_identity(first_page: u64, page_count: u64,
                addr_space: &mut paging::AddrSpace,
                alloc: &mut memory::FrameAllocator) {
    for i in first_page..first_page+page_count {
        addr_space.map_to(paging::Page(i), paging::Frame(i), 0b1000, alloc);
    }
}

#[no_mangle]
pub extern fn loader_entry(mbinfop: *const multiboot::Info) {
    let mbinfo = unsafe { &*mbinfop };
    let mod_raw_entries = unsafe {
        from_raw_parts(mbinfo.mods_addr as *const ModuleRaw, mbinfo.mods_count as usize)
    };
    let mut mod_entries = mod_raw_entries.into_iter().map(Module::from_raw);
    // Kernel should be first (and only) module.
    let kernel_mod = mod_entries.next().expect("Kernel module not loaded.");
    let elf_header: &elf::ElfHeaderRaw = read_from_buffer(kernel_mod.data, 0);

    // Set up memory map and allocator.
    let loader_extent = get_loader_extent(mbinfo);
    let mut mem_map = memory::MemoryMap::from_multiboot(mbinfo);
    write_terminal(format_args!("{:x} {:x}", loader_extent.0, loader_extent.1));
    mem_map.reserve(loader_extent.0, loader_extent.1);
    mem_map.reserve(kernel_mod.data.as_ptr() as u64, kernel_mod.data.len() as u64);
    mem_map.reserve(mbinfop as u64, size_of::<multiboot::Info>() as u64);
    for i in 0..mem_map.num_entries {
        write_terminal(format_args!("{:x} {:x}", mem_map.entries[i].base, mem_map.entries[i].length));
    }
    let mut alloc = memory::FrameAllocator::new(&mem_map);

    // Set up paging for kernel.
    let mut addr_space = paging::AddrSpace::new(&mut alloc);
    map_kernel(kernel_mod.data, &mut addr_space, &mut alloc);
    // Identity map first 16 MiB
    map_identity(0, 4096, &mut addr_space, &mut alloc);

    // Switch to 64 bit and call kernel.
    let mbinfo_addr = mbinfop as u64;
    let page_table_addr = addr_space.get_p4_addr();
    let kernel_entry_addr = elf_header.entry;

    unsafe { kernel_handoff(&mbinfo_addr as *const u64,
                            &page_table_addr as *const u64,
                            &kernel_entry_addr as *const u64); }

    loop { }
}
