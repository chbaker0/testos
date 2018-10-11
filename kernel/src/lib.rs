#![feature(abi_x86_interrupt)]
#![feature(alloc)]
#![feature(alloc_error_handler)]
#![feature(allocator_api)]
#![feature(asm)]
#![feature(const_fn)]
#![feature(core_panic_info)]
#![feature(integer_atomics)]
#![feature(lang_items)]
#![no_std]

extern crate alloc;
#[macro_use]
extern crate intrusive_collections;
#[macro_use]
extern crate lazy_static;
#[macro_use]
extern crate log;
extern crate shared;
extern crate spin;
extern crate x86_64;

use core::cell;
use core::fmt::write;
use core::ops::DerefMut;
use core::panic;
use core::str::from_utf8;
use shared::handoff;
use shared::multiboot;

mod acpi;
mod interrupts;
mod logging;
mod mm;
mod sched;
mod selftest;
mod sync;
mod terminal;
mod vga;

#[global_allocator]
static ALLOCATOR: mm::GlobalAllocator = unsafe { mm::GlobalAllocator::new() };

static TERMBUF: spin::Mutex<terminal::Buffer> = spin::Mutex::new(terminal::Buffer::new());

#[panic_handler]
#[no_mangle]
pub extern "C" fn panic_fmt(info: &panic::PanicInfo) -> ! {
    info!("{}", info);
    loop {
        unsafe {
            asm!("hlt");
        }
    }
}

#[alloc_error_handler]
fn alloc_handler(_: core::alloc::Layout) -> ! {
    panic!("Failed alloc");
}

#[no_mangle]
pub extern "C" fn kinit(_mbinfop: *const multiboot::Info, boot_infop: *const handoff::BootInfo) {
    let boot_info: handoff::BootInfo = unsafe { (*boot_infop).clone() };

    logging::init();

    info!("Memory map:");
    let mem_map = &boot_info.mem_map;
    for i in 0..mem_map.num_entries as usize {
        let entry = &mem_map.entries[i];
        // We need to do this to avoid borrowing packed fields
        let base = entry.base;
        let length = entry.length;
        info!("    Address {:x} Size {:x}", base, length);
    }

    interrupts::init();
    mm::init(mem_map.clone());
    acpi::init();

    selftest::run_tests();

    sched::init();
    sched::spawn(thread1);
    sched::spawn(thread2);
    sched::spawn(thread3);
    loop {
        sched::yield_cur();
        SEMAPHORE.signal();
    }

    panic!("Context switched back to kinit");
}

lazy_static! {
    static ref SEMAPHORE: sync::Semaphore = { sync::Semaphore::new(0) };
}

pub extern "C" fn thread1() -> ! {
    loop {
        SEMAPHORE.wait();
        info!("1");
    }
}

pub extern "C" fn thread2() -> ! {
    loop {
        SEMAPHORE.wait();
        info!("2");
    }
}

pub extern "C" fn thread3() -> ! {
    loop {
        SEMAPHORE.wait();
        info!("3");
    }
}
