# testos

A hobby x86_64 OS kernel written in Rust (`no_std`, nightly-only toolchain).
See [README.md](README.md) for build/run commands and project layout — this
file covers things the README doesn't: current state, gotchas, and how to
verify changes.

## Current state (as of 2026-07-19)

The project just finished migrating its boot process from Multiboot2/GRUB to
a custom UEFI loader. The loader (`loader/`) now successfully parses the
kernel ELF, maps its segments, sets up paging, and jumps into
`kernel_entry` in [src/kmain.rs](src/kmain.rs).

**`kernel_entry` itself is almost entirely commented out right now** — it
prints one line to the debugcon and halts. GDT/IDT setup, the frame
allocator, and the scheduler init that used to run here (visible as commented
-out code) have not yet been re-wired for the new UEFI handoff. This is the
active work-in-progress; don't assume any of that subsystem is currently
exercised at boot even though the code for it still exists in `src/`.

## Build/run essentials

* Nightly Rust only, with `-Zbuild-std`; see `.cargo/config.toml` for the
  real cargo aliases (`kbuild`, `kcheck`, `scheck`, `icheck`, `lcheck`, etc.)
  — each targets a different crate/target triple, see README for the map.
* `./make-image.sh` builds everything and assembles `out/esp` (a UEFI ESP
  directory, **not** an ISO). `./run-qemu.sh` boots it in QEMU.
* Both scripts `source .env` and will hard-fail (`set -e`) if it doesn't
  exist. Copy `.env.example` to `.env` (can be empty) before running them.
* `run-qemu.sh` forwards extra args to `qemu-system-x86_64`, e.g.
  `./run-qemu.sh -s -S` to wait for a debugger.

## Verifying changes

There's no test suite beyond `cargo stest` (unit tests in `shared`, the one
crate that doesn't need to run in kernel space). Everything else — kernel,
loader, init — can only really be checked by booting it. Prefer, in order:

1. `cargo kcheck` / `cargo lcheck` / `cargo icheck` (fast compile check).
2. Actually boot it and read the output. The kernel writes to the QEMU
   debugcon (port 0xE9, see [shared/src/log.rs](shared/src/log.rs)), so you
   can run headlessly and capture output instead of needing a display:
   ```
   qemu-system-x86_64 \
     -drive if=pflash,format=raw,readonly=on,file=target/ovmf/x64/code.fd \
     -drive if=pflash,format=raw,readonly=on,file=target/ovmf/x64/vars.fd \
     -drive format=raw,file=fat:rw:out/esp \
     -debugcon stdio -display none
   ```
   Run this with a timeout (QEMU won't exit on its own after the kernel
   halts) and read stdout for panics/log output.

## Known rough edges

* `mkimage/` (builds a GRUB/xorriso ISO) is a leftover from the pre-UEFI boot
  flow and isn't invoked by `make-image.sh` anymore. Don't assume it's part
  of the live build path.
* `targets/x86_64-unknown-none.json` (kernel) and
  `targets/x86_64-unknown-testos.json` (init) differ in more than name —
  e.g. soft-float/no-SSE + `code-model: kernel` vs. SSE enabled +
  `code-model: small`. This looks intentional (init presumably runs more
  like normal userspace code) but hasn't been confirmed — don't assume it's
  a mistake, and don't assume it's deliberate either without checking with
  the user if it becomes relevant.
