#![deny(unsafe_op_in_unsafe_fn)]
#![feature(abi_x86_interrupt)]
#![no_std]
#![no_main]

mod gdt;
mod idt;
mod kmain;
mod mm;
mod pic;
mod sched;
