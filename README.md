# Test OS

A useless OS in Rust

## Building

### Prerequisites

* Recent nightly Rust toolchain with the `rust-src` component
* Recent nightly Cargo
* Xorriso to generate bootable ISOs (currently the only way to run testos)

For Rust, see https://rustup.rs/ or use your system package manager. For
Xorriso, use your system package manager (not sure how to get it on Windows).

### Build

Use the following commands:
* `cargo kimage`: builds the bootable ISO.
* `cargo kcheck`: runs `cargo check` on the kernel source.
* `cargo kclippy`: runs `cargo clippy` on the kernel source.
* `cargo scheck` & `cargo sclippy`: equivalents for code in shared.
* `cargo stest`: run unit tests in shared.

.cargo/config.toml defines the aliases above.

### Run

QEMU is the main supported way to run testos. It is currently not tested on real
hardware. After running `cargo mkimage` you can run it with

```qemu-system-x86_64 -cdrom out/kernel.iso```

## Project structure

The project is organized into multiple packages in a Cargo workspace. The main
package (whose source is in src) is the kernel itself. The sub-packages are:
* **shared**: Standalone types and helpers that don't need to be run in kernel
  space. This was originally called "shared" because a previous iteration had an
  intermediate bootloader that had to be compiled separately. Now it's only
  separate to make it easier to run unit tests.
* **mkimage**: Builds a bootable ISO from the built kernel using GRUB and
  xorriso.
* **buildutil**: Helpers shared between build scripts and mkimage.

**targets** contains target specifications passed to rustc. Currently there is
only one: x86_64-unknown-none.json. This is necessary to target bare-metal x86.

**out** contains output artifacts, including the bootable kernel.iso.

A prebuilt GRUB image is located in **third_party**.
