#![deny(unsafe_op_in_unsafe_fn)]
#![feature(abi_x86_interrupt)]
#![cfg_attr(not(test), no_std)]
#![cfg_attr(not(test), no_main)]

mod mm;

#[cfg(not(test))]
mod gdt;
#[cfg(not(test))]
mod idt;
#[cfg(not(test))]
mod kmain;
#[cfg(not(test))]
mod pic;
