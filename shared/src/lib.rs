//! Shared utilities and self-contained helpers
//!
//! This crate contains code that is either shared by both the kernel and
//! loader, or is fairly self-contained. Unit testing is a big motivation for
//! this crate.

#![cfg_attr(not(test), no_std)]

pub mod physmem;
