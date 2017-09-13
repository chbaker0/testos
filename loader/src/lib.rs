#![feature(lang_items)]
#![no_std]

extern crate rlibc;
extern crate shared;

use core::mem::size_of;
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
    loop { }
}
