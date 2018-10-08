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
mod mm;
mod interrupts;
mod sched;
mod sync;
mod terminal;
mod vga;

#[global_allocator]
static ALLOCATOR: mm::GlobalAllocator = unsafe { mm::GlobalAllocator::new() };

// C kernel functions.
extern {
    pub fn print_line(str: *const u8);
}

static TERMBUF: spin::Mutex<terminal::Buffer> = spin::Mutex::new(terminal::Buffer::new());

fn log_terminal(s: &str)
{
    let mut termbuf = TERMBUF.lock();
    termbuf.write_line(s);
    vga::display_terminal(termbuf.deref_mut());
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
    let _ = write(&mut buf_writer, args);
    buf_writer.buffer[79] = 0;

    match from_utf8(&buf_writer.buffer) {
        Ok(s) => log_terminal(s),
        Err(_) => panic!(),
    }
}

#[panic_handler]
#[no_mangle]
pub extern fn panic_fmt(_info: &panic::PanicInfo) -> ! {
/*
    let mut buf_writer = BufWriter::new();
    let _ = write(&mut buf_writer, format_args!("Panic in {} at line {}: ", file, line));
    let _ = write(&mut buf_writer, panic_args);
    buf_writer.buffer[79] = 0;

    match from_utf8(&buf_writer.buffer) {
        Ok(s) => log_terminal(s),
        Err(_) => (), // We're already panicking, there's nothing else to do.
    }
*/
    loop { unsafe { asm!("hlt"); } }
}

#[alloc_error_handler]
fn alloc_handler(_: core::alloc::Layout) -> ! {
    panic!("fml haha");
}

#[no_mangle]
pub extern fn kinit(_mbinfop: *const multiboot::Info, boot_infop: *const handoff::BootInfo) {
    let boot_info: handoff::BootInfo = unsafe { (*boot_infop).clone() };

    log_terminal("Memory map:");
    let mem_map = &boot_info.mem_map;
    for i in 0..mem_map.num_entries as usize {
        let entry = &mem_map.entries[i];
        write_terminal(format_args!("    Address {:x} Size {:x}", entry.base, entry.length));
    }

    interrupts::init();
    mm::init(mem_map.clone());
    acpi::init();

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
    static ref SEMAPHORE: sync::Semaphore = {
        sync::Semaphore::new(0)
    };
}

pub extern fn thread1() -> ! {
    loop {
        SEMAPHORE.wait();
        write_terminal(format_args!("1"));
    }
}

pub extern fn thread2() -> ! {
    loop {
        SEMAPHORE.wait();
        write_terminal(format_args!("2"));
    }
}

pub extern fn thread3() -> ! {
    loop {
        SEMAPHORE.wait();
        write_terminal(format_args!("3"));
    }
}
