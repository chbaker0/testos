#![feature(lang_items)]
#![no_std]

extern crate rlibc;

// C kernel functions.
extern {
    pub fn print_line(str: *const u8);
}

#[lang="panic_fmt"]
#[no_mangle]
pub extern fn panic_fmt(_: ::core::fmt::Arguments, _: &'static str, _: u32) -> ! {
    unsafe {
        print_line("Panic\0".as_ptr());
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
