#![feature(const_fn)]
#![feature(lang_items)]
#![no_std]

extern crate rlibc;

use core::cell;
use core::cmp;
use core::fmt::Write;
use core::fmt::write;
use core::ops::DerefMut;
use core::option::Option;
use core::str::from_utf8;

mod elf;
mod mm;
mod multiboot;
mod terminal;
mod vga;

// C kernel functions.
extern {
    pub fn print_line(str: *const u8);
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

fn kernel_image_bounds(mbinfo: &multiboot::Info) -> (u64, u64) {
    let symtab_info = multiboot::get_section_header_table_info(mbinfo);
    (0..symtab_info.entry_count)
        .map(|ndx| unsafe { elf::get_section_header(symtab_info.addr, symtab_info.entry_size, ndx) })
        .map(|header| (header.addr as u64, (header.addr + header.size) as u64))
        .filter(|&(lower, upper)| upper - lower > 0)
        .fold((u64::max_value(), 0), |(a, b), (c, d)| (cmp::min(a, c), cmp::max(b, d)))
}

#[no_mangle]
pub extern fn rustmain(mbinfop: *const multiboot::Info) {
    let mbinfo: &multiboot::Info = unsafe { &*mbinfop };
    assert!(mbinfo.flags & multiboot::INFO_FLAG_MMAP > 0);

    log_terminal("Memory map:");
    for entry in multiboot::get_memory_map_iterator(mbinfo) {
        write_terminal(format_args!("    Address {:x} Size {:x}", entry.base_addr, entry.length));
    }

    // Calculate extent of kernel in memory.
    let (kernel_lower, kernel_upper) = kernel_image_bounds(&mbinfo);
    write_terminal(format_args!("Kernel starts at {:x} and ends at {:x}.", kernel_lower, kernel_upper));
    mm::init(mbinfo);

    let frame1 = mm::get_frame_allocator().get_frame();
    let frame2 = mm::get_frame_allocator().get_frame();
    write_terminal(format_args!("Allocated frames at {:x} and {:x}.", frame1, frame2));

    loop { }
}
