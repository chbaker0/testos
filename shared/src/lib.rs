//! Shared utilities and self-contained helpers
//!
//! This crate contains code that is either shared by both the kernel and
//! loader, or is fairly self-contained. Unit testing is a big motivation for
//! this crate.
//!
#![feature(allocator_api)]
#![feature(const_option)]
#![feature(int_roundings)]
#![feature(maybe_uninit_slice)]
#![feature(pointer_byte_offsets)]
#![feature(pointer_is_aligned)]
#![feature(ptr_metadata)]
#![feature(slice_ptr_len)]
#![deny(unsafe_op_in_unsafe_fn)]
#![cfg_attr(not(test), no_std)]

#[cfg(feature = "alloc")]
extern crate alloc;

#[cfg(test)]
extern crate std;

pub mod memory;
pub mod vga;
