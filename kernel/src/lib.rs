#![feature(abi_x86_interrupt)]
#![feature(alloc)]
#![feature(allocator_api)]
#![feature(asm)]
#![feature(const_fn)]
#![feature(const_ptr_null_mut)]
#![feature(const_refcell_new)]
#![feature(global_allocator)]
#![feature(iterator_step_by)]
#![feature(lang_items)]
#![feature(unique)]
#![no_std]

extern crate alloc;
#[macro_use]
extern crate intrusive_collections;
#[macro_use]
extern crate lazy_static;
extern crate rlibc;
extern crate shared;
extern crate spin;
extern crate x86_64;

use core::cell;
use core::fmt::write;
use core::ops::DerefMut;
use core::ptr::null_mut;
use core::str::from_utf8;
use shared::handoff;
use shared::multiboot;

mod acpi;
mod context;
mod mm;
mod interrupts;
mod terminal;
mod vga;

#[global_allocator]
static ALLOCATOR: mm::GlobalAllocator = unsafe { mm::GlobalAllocator::new() };

// C kernel functions.
extern {
    pub fn print_line(str: *const u8);
    pub fn context_init(stack: *mut u8, entry: extern fn() -> !) -> *mut u8;
    pub fn context_switch(stack: *mut u8, stack: *mut *mut u8);
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
    let _ = write(&mut buf_writer, args);
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
    let _ = write(&mut buf_writer, format_args!("Panic in {} at line {}: ", file, line));
    let _ = write(&mut buf_writer, panic_args);
    buf_writer.buffer[79] = 0;

    match from_utf8(&buf_writer.buffer) {
        Ok(s) => log_terminal(s),
        Err(_) => (), // We're already panicking, there's nothing else to do.
    }

    loop { unsafe { asm!("hlt"); } }
}

const STACK_PAGES: u64 = 1024;

fn allocate_kernel_stack() -> *mut u8 {
    let first_page = mm::allocate_address_space(STACK_PAGES).unwrap();
    for i in 0..STACK_PAGES {
        let frame = mm::get_frame_allocator().get_frame() as u64;
        mm::map_to(mm::Page(first_page + i), mm::Frame(frame >> 12), 0b1001, mm::get_frame_allocator());
    }
    ((first_page + STACK_PAGES) << 12) as *mut u8
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

    let stack = allocate_kernel_stack();
    unsafe {
        let adj_stack = context_init(stack, kmain);
        let mut old_stack = null_mut();
        context_switch(adj_stack, &mut old_stack as *mut _);
    }

    panic!("Context switched back to kinit");
}

pub extern fn kmain() -> ! {
    write_terminal(format_args!("In kmain"));

    loop { unsafe { asm!("hlt"); } }
}
