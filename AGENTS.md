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
* A fresh toolchain needs both `rustup component add rust-src` **and**
  `rustup target add x86_64-unknown-uefi` — the loader targets that
  built-in triple directly (not one of the custom `targets/*.json` files,
  which only need `rust-src` + `-Zbuild-std`). Missing the target add gives
  a `can't find crate for `core`` error that's easy to misread as something
  else.
* Because of this, `rustup show` / `rustup target list --installed` will
  only ever list `x86_64-unknown-uefi` (and the host triple) — it will
  never show `x86_64-unknown-none` or `x86_64-unknown-testos`, since those
  are custom `targets/*.json` specs built via `-Zbuild-std`, not
  rustup-managed targets. Their absence from that output is expected, not
  a sign of a broken or incomplete toolchain install.
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
   halts) and read stdout for panics/log output. **Bound it externally:**
   there's no `timeout`/`gtimeout` on macOS, and a `perl -e 'alarm N; exec
   ...'` wrapper does *not* work — QEMU installs its own `SIGALRM` handler
   and ignores it. Background QEMU and kill it from outside:
   ```
   ... -debugcon stdio -display none >log 2>/dev/null &
   pid=$!; ( sleep 45; pkill -KILL -f qemu-system-x86_64 ) & wait $pid; cat log
   ```
   **Budget > 40s for a full boot.** A run currently spends ~30-40s (under
   QEMU TCG on Apple Silicon) between the `kernel loaded and mapped` and
   `identity mapped existing memory` debugcon lines — that's the known
   inefficiency tracked in **issue #5** (the loader identity-maps the entire
   UEFI memory map, including a multi-GiB reserved region, at 4 KiB
   granularity), *not* a hang. Don't mistake it for a regression.

## Known rough edges

* `rust-toolchain` pins bare `nightly` with no date, so a fresh install can
  land on a nightly with breaking changes to *unstable* APIs this project
  relies on (`-Zbuild-std`, custom target-spec fields, unstable trait impls
  in dependencies, etc.). If a fresh checkout fails to compile with an
  error that doesn't look related to anything recently changed here — a
  trait-impl mismatch in a dependency, a target-spec field rejected as
  invalid — suspect nightly drift before suspecting the code. Check for a
  newer version of the offending crate or a renamed target-spec value
  first; don't assume it's a regression in this repo.
* `mkimage/` (builds a GRUB/xorriso ISO) is a leftover from the pre-UEFI boot
  flow and isn't invoked by `make-image.sh` anymore. Don't assume it's part
  of the live build path.
* `cargo`/`rustc`/`rustup`/`rust-analyzer` live in `~/.cargo/bin`, which is
  not always on `PATH` in a fresh shell session (depends on whether
  `~/.cargo/env` got sourced). If these commands report "not found," that's
  almost always a stale/non-interactive shell rather than a broken or
  missing toolchain install — check `ls ~/.cargo/bin` before concluding
  anything is actually missing, and ask the user to confirm/restart the
  shell rather than adding PATH exports or wrapper scripts.
* `cargo stest` builds `shared` for the **host** target, which on this repo's
  dev machine is aarch64 (Apple Silicon) — not x86. So `shared` must stay
  host-buildable: any x86-only code (e.g. the QEMU debugcon port write in
  [shared/src/log.rs](shared/src/log.rs), which uses `x86_64::instructions`,
  gated to `target_arch = "x86_64"`) must be `#[cfg(target_arch = "x86_64")]`-
  gated and a no-op elsewhere, or the whole test suite fails to compile. Fix
  such breakage at the source rather than working around it on the host (e.g.
  don't add a Rosetta `x86_64-apple-darwin` target just to run the tests).
* `targets/x86_64-unknown-none.json` (kernel) and
  `targets/x86_64-unknown-testos.json` (init) differ in more than name —
  e.g. soft-float/no-SSE + `code-model: kernel` vs. SSE enabled +
  `code-model: small`. This looks intentional (init presumably runs more
  like normal userspace code) but hasn't been confirmed — don't assume it's
  a mistake, and don't assume it's deliberate either without checking with
  the user if it becomes relevant.
