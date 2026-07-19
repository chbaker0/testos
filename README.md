# Test OS

A useless OS in Rust

## Building

testos boots via UEFI: a custom loader (**loader**) reads the kernel ELF off
the boot volume, maps its segments, sets up paging, and jumps to it. Booting
via GRUB/Multiboot2 has been removed.

### Prerequisites

* Recent nightly Rust toolchain with the `rust-src` component (see
  `rust-toolchain`)
* QEMU (`qemu-system-x86_64`) to run testos — this is currently the only
  tested way to run it, real hardware is untested
* A `.env` file at the repo root (see below) — `make-image.sh` and
  `run-qemu.sh` both `source` it and will fail without one, even if it's empty

For Rust, see https://rustup.rs/ or use your system package manager.

### Build & run

```
./make-image.sh   # fetches OVMF prebuilts, builds the loader + kernel, assembles out/esp
./run-qemu.sh      # boots out/esp in QEMU using the fetched OVMF firmware
```

`make-image.sh` produces a UEFI ESP directory at `out/esp` (not an ISO):
`out/esp/efi/boot/bootx64.efi` is the loader, `out/esp/testos` is the kernel
ELF. `run-qemu.sh` boots that ESP directory directly with OVMF pflash images
fetched into `target/ovmf`.

`.env` can set (all optional, shown with their defaults):
```
TESTOS_QEMU_FLAGS=            # extra flags, e.g. "-debugcon stdio -display none" for headless output
TESTOS_QEMU_EFI_CODE=target/ovmf/x64/code.fd
TESTOS_QEMU_EFI_VARS=target/ovmf/x64/vars.fd
```

Useful `cargo` aliases (defined in `.cargo/config.toml`):
* `cargo kbuild` / `kcheck` / `kclippy` / `kfix`: kernel (this workspace's
  root package), targeting `targets/x86_64-unknown-none.json`.
* `cargo icheck` / `iclippy`: **init**, targeting
  `targets/x86_64-unknown-testos.json`.
* `cargo lcheck` / `lclippy`: **loader**, targeting the built-in
  `x86_64-unknown-uefi` target.
* `cargo scheck` / `sclippy` / `stest`: **shared** — the only crate with
  runnable unit tests, since everything else needs a booted kernel to
  exercise.

## Project structure

The project is organized into multiple packages in a Cargo workspace. The main
package (whose source is in `src`) is the kernel itself. The sub-packages are:
* **shared**: Standalone types and helpers that don't need to be run in kernel
  space (memory/address types, paging structures, logging). Kept separate so
  it can have normal unit tests.
* **loader**: The UEFI bootloader. Loads the kernel ELF, maps its segments,
  and jumps to `_start`.
* **init**: The first userspace program the kernel loads (early/WIP).
* **mkimage**: Builds a bootable GRUB/xorriso ISO. Left over from the
  pre-UEFI boot flow and not currently invoked by `make-image.sh`.
* **buildutil**: Helpers shared between build scripts and mkimage.
* **fetch-prebuilts**: Downloads prebuilt OVMF firmware (UEFI firmware for
  QEMU) into `target/ovmf`.

**targets** contains target specifications passed to rustc:
`x86_64-unknown-none.json` for the kernel, `x86_64-unknown-testos.json` for
init.

**out** contains build output, including the `out/esp` UEFI boot volume.
