#![feature(lang_items)]
#![no_std]

extern crate rlibc;

#[lang="panic"]
#[no_mangle]
pub extern fn panic_fmt(_: ::core::fmt::Arguments, _: &'static str, _:u32) -> ! {
    loop { }
}

#[no_mangle]
pub extern fn loader_entry() {
    loop { }
}
