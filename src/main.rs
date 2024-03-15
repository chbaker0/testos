#![deny(unsafe_op_in_unsafe_fn)]
#![feature(abi_x86_interrupt)]
#![feature(naked_functions)]
#![no_std]
#![no_main]
#![allow(unused)]

extern crate alloc;

mod gdt;
mod idt;
mod kmain;
mod mm;
mod pic;
mod sched;

pub use kmain::kernel_entry;

fn halt_loop() -> ! {
    loop {
        x86_64::instructions::hlt();
    }
}
