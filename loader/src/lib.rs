#![feature(const_fn)]
#![feature(lang_items)]
#![no_std]

extern crate rlibc;
extern crate shared;

mod terminal;
mod vga;

use core::cell;
use core::fmt::write;
use core::mem::size_of;
use core::ops::DerefMut;
use core::slice::from_raw_parts;
use core::str::from_utf8;
use shared::*;

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

    // Check that the kernel image is what we expect.
    assert!(elf_header.ident[0] == 0x7f
            && elf_header.ident[1] == 'E' as u8
            && elf_header.ident[2] == 'L' as u8
            && elf_header.ident[3] == 'F' as u8);
    assert!(elf_header.typ == elf::ElfType::Exec as u16);
    assert!(elf_header.machine == 62);

    // Display segments.


    loop { }
}
