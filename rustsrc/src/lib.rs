#![feature(const_fn)]
#![feature(lang_items)]
#![no_std]

extern crate rlibc;

use core::cell;
use core::fmt::Write;
use core::fmt::write;
use core::ops::DerefMut;
use core::str::from_utf8;

mod multiboot;
mod terminal;
mod vga;

// C kernel functions.
extern {
    pub fn print_line(str: *const u8);
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

struct PanicWriter {
    buffer: [u8; 80],
    ndx: usize,
}

impl core::fmt::Write for PanicWriter {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        let trunc = s.bytes().take(80-self.ndx);
        for c in trunc {
            self.buffer[self.ndx] = c;
            self.ndx += 1;
        }
        Ok(())
    }
}

#[lang="panic_fmt"]
#[no_mangle]
pub extern fn panic_fmt(_: ::core::fmt::Arguments, file: &'static str, line: u32) -> ! {
    let mut panic_writer = PanicWriter {
        buffer: [0; 80],
        ndx: 0,
    };
    write(&mut panic_writer, format_args!("Panic in {} at line {}\0", file, line));
    panic_writer.buffer[79] = 0;

    match from_utf8(&panic_writer.buffer) {
        Ok(s) => log_terminal(s),
        Err(_) => (), // We're already panicking, there's nothing else to do.
    }

    loop { }
}

#[no_mangle]
pub extern fn rustmain(mbinfop: *const multiboot::Info) {
    let mbinfo: &multiboot::Info = unsafe { &*mbinfop };

    if mbinfo.flags & multiboot::INFO_FLAG_MMAP > 0 {
        log_terminal("Memory map present.");
    }

    vga::clear();

    log_terminal("Test");

    loop { }
}
