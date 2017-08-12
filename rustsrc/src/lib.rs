#![feature(lang_items)]
#![no_std]

extern crate rlibc;

use core::fmt::Write;
use core::fmt::write;

// C kernel functions.
extern {
    pub fn print_line(str: *const u8);
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
    unsafe {
        print_line(&panic_writer.buffer as *const u8);
    }
    loop { }
}

#[no_mangle]
pub extern fn rustmain() {
    unsafe {
        print_line("Test from Rust!\0".as_ptr())
    }
    panic!();
}
