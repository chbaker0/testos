#![feature(lang_items)]
#![no_std]

extern crate rlibc;
extern crate shared;

use core::slice::from_raw_parts;
use shared::*;

#[lang="panic_fmt"]
#[no_mangle]
pub extern fn panic_fmt(_: ::core::fmt::Arguments, _: &'static str, _:u32) -> ! {
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

#[no_mangle]
pub extern fn loader_entry(mbinfop: *const multiboot::Info) {
    let mbinfo = unsafe { &*mbinfop };
    let mod_raw_entries = unsafe {
        from_raw_parts(mbinfo.mods_addr as *const ModuleRaw, mbinfo.mods_count as usize)
    };
    let mod_entries = mod_raw_entries.into_iter().map(Module::from_raw);
    loop { }
}
