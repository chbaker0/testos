# Test OS

A useless OS in Rust

## Project structure

* **loader**: A second stage bootloader. Gets loaded by GRUB as a 32-bit multiboot image, switches to 64-bit mode, and loads the main kernel (soon)
* **kernel**: Will be the main 64-bit kernel
* **targets**: Target files for rustc. Describe how to generate code for a bare-metal target

## Building
First, install the latest nightly cargo and rustc. Building requires several unstable features of both cargo and rustc.

To build the loader, simply run `cargo build` in `loader`.
