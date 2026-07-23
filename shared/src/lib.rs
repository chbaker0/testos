//! Shared utilities and self-contained helpers
//!
//! This crate contains code that is either shared by both the kernel and
//! loader, or is fairly self-contained. Unit testing is a big motivation for
//! this crate.
//!
#![feature(allocator_api)]
#![feature(pointer_is_aligned_to)]
#![feature(ptr_metadata)]
#![cfg_attr(not(test), no_std)]

#[cfg(feature = "alloc")]
extern crate alloc;

#[cfg(test)]
extern crate std;

pub mod boot_info;
pub mod log;
pub mod memory;
pub mod vga;
