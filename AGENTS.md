# testos

A hobby x86_64 OS kernel written in Rust (`no_std`, nightly-only toolchain). See
[README.md](README.md) for the package layout and cargo alias reference. This
file covers knowledge which agents need but can't easily infer from reading the code cold:
* Goals and roadmap sketch
* Current project status
* Project structure
* Environment/build gotchas
* Tooling / how to verify changes
* Other conventions

This is a learning project, so expect lots of brainstorming, experimentation,
trial-and-error, and exploration of existing tools and practices. I'm also
learning how to use Claude Code in this existing project, so keep that in mind.

## Long-term goal

A almost fully pure Rust kernel, written in idiomatic Rust, using the latest in
safety tooling.

I don't have an exact architecture in mind but I lean towards a microkernel-y
based system with a schema-based RPC and syscall system.

## Next steps

### Explore build systems

Right now, everything is a mix of scripts run at build time (which mostly invoke cargo, a couple special tools, and compost everything together into a bootable image), but the script hides a lot. Would it be appropriate to use a different way to orchestrate the build, which makes the relationships and stages more visible?

### GitHub actions

Actions for the main verification steps will:
* Speed up dev cycle on Apple silicon
* Improve build hermeticity
* Allow more automation

### C toolchain

Eventually, a C toolchain necessary. However, it is helpful much sooner:
* ACPI is close on the horizon, and using ACIPCA would make my life way easier
* Heap algorithms written in C

### "Full" system setup (later)

* ACPI, PCI, turning off all legacy layers, etc

## Project status

Last updated 2026-07-19. The project just finished migrating its boot
process from Multiboot2/GRUB to a custom UEFI loader. The loader
(`loader/`) now successfully parses the kernel ELF, maps its segments, sets
up paging, and jumps into `kernel_entry` in [src/kmain.rs](src/kmain.rs).

**`kernel_entry` itself is almost entirely commented out right now** — it
prints one line to the debugcon and halts. GDT/IDT setup, the frame
allocator, and the scheduler init that used to run here (visible as
commented-out code) have not yet been re-wired for the new UEFI handoff.
This is the active work-in-progress; don't assume any of that subsystem is
currently exercised at boot even though the code for it still exists in
`src/`.

A dedicated testing/verification strategy (beyond ad hoc debugcon-reading
and manual QEMU boots) is wanted before more kernel feature work lands —
GDB-over-QEMU-stub debugging is real but slow here (see "Verifying
changes" below), which is part of the motivation.

## Environment setup

* Nightly Rust only, with `-Zbuild-std`. `rust-toolchain` pins bare
  `nightly` with **no date**, so a fresh install can land on a nightly with
  breaking changes to the unstable APIs this project relies on
  (`-Zbuild-std`, custom target-spec fields, unstable trait impls in
  dependencies). If a fresh checkout fails to compile with an error that
  doesn't look related to anything recently changed here — a trait-impl
  mismatch in a dependency, a target-spec field rejected as invalid —
  suspect nightly drift before suspecting the code. Check for a newer
  version of the offending crate or a renamed target-spec value first;
  don't assume it's a regression in this repo.
* A fresh toolchain needs **both** `rustup component add rust-src` **and**
  `rustup target add x86_64-unknown-uefi` — the loader targets that
  built-in triple directly. Missing the target add gives a
  `can't find crate for \`core\`` error that's easy to misread as something
  else. The other two targets (`targets/x86_64-unknown-none.json` for the
  kernel, `targets/x86_64-unknown-testos.json` for init) are custom
  `-Zbuild-std` specs, not rustup-managed, so they will **never** show up
  in `rustup target list --installed` — that absence is expected, not a
  sign of a broken install.
* `cargo`/`rustc`/`rustup`/`rust-analyzer` live in `~/.cargo/bin`, which is
  not always on `PATH` in a fresh shell (depends on whether `~/.cargo/env`
  got sourced). If these commands report "not found," that's almost always
  a stale/non-interactive shell, not a missing install — check
  `ls ~/.cargo/bin` before concluding anything is actually missing.
* Both `make-image.sh` and `run-qemu.sh` `source .env` and hard-fail
  (`set -e`) if it doesn't exist. Copy `.env.example` to `.env` (can be
  empty) before running them.
* If a command that should exist isn't found, or the toolchain misbehaves
  in a way that suggests a broken/incomplete install, **stop and ask** how
  to proceed rather than routing around it with shims, wrapper scripts, or
  persistent PATH edits. Read-only diagnostics (`which`, `ls` on expected
  install dirs) are fine to gather facts first; installing anything or
  patching config is not.

## Build & run

```
./make-image.sh   # fetches OVMF prebuilts, builds the loader + kernel, assembles out/esp
./run-qemu.sh      # boots out/esp in QEMU using the fetched OVMF firmware
```

`make-image.sh` produces a UEFI ESP directory at `out/esp` (not an ISO).
`run-qemu.sh` forwards extra args to `qemu-system-x86_64`, e.g.
`./run-qemu.sh -s -S` to wait for a debugger.

Cargo aliases (see `.cargo/config.toml` for the full definitions):

| Alias | Crate | Target |
|---|---|---|
| `kbuild` / `kcheck` / `kclippy` / `kfix` | kernel (root package) | `targets/x86_64-unknown-none.json` |
| `icheck` / `iclippy` | init | `targets/x86_64-unknown-testos.json` |
| `lcheck` / `lclippy` | loader | built-in `x86_64-unknown-uefi` |
| `scheck` / `sclippy` / `stest` / `smiri` | shared | host triple |

## Project structure

* **src**: the kernel itself (root package).
* **shared**: standalone types/helpers that don't need kernel space
  (memory/address types, paging structures, logging) — kept separate so it
  can have normal host-side unit tests.
* **loader**: the UEFI bootloader. Loads the kernel ELF, maps its segments,
  sets up paging, jumps to `_start`.
* **init**: the first userspace program the kernel loads (early/WIP).
* **mkimage**: builds a GRUB/xorriso ISO. Leftover from the pre-UEFI boot
  flow — **not** invoked by `make-image.sh` anymore; don't assume it's part
  of the live build path.
* **buildutil**: helpers shared between build scripts and mkimage.
* **fetch-prebuilts**: downloads prebuilt OVMF firmware into `target/ovmf`.
* **targets**: target specs passed to rustc. `x86_64-unknown-none.json`
  (kernel) and `x86_64-unknown-testos.json` (init) differ in more than
  name — e.g. soft-float/no-SSE + `code-model: kernel` vs. SSE enabled +
  `code-model: small`. This looks intentional (init presumably runs more
  like normal userspace code) but hasn't been confirmed — don't assume
  it's a mistake, and don't assume it's deliberate either without checking
  with the user if it becomes relevant.

## Verifying changes

There's no test suite beyond `cargo stest` (host unit tests in `shared`,
the one crate that doesn't need kernel space to run). Everything else —
kernel, loader, init — can only really be checked by booting it, which is
slow on Apple Silicon (see "Booting headlessly" below) — part of the
motivation for the GitHub Actions item under "Next steps" above. In order
of preference:

1. `cargo kcheck` / `cargo lcheck` / `cargo icheck` — fast compile check.
2. `cargo stest` — unit tests, including an end-to-end `harness_tests` in
   [shared/src/memory/paging.rs](shared/src/memory/paging.rs) that drives
   the real `Mapper` against a fake physical-memory arena and checks each
   mapping with an independent `translate` oracle, exercising the
   multi-level page-table walk, parent-table allocation/reuse, and flag
   masking without booting.

   Note `shared` builds for the **host** target, which on this dev machine
   is aarch64 (Apple Silicon), not x86. Any x86-only code (e.g. the QEMU
   debugcon port write in [shared/src/log.rs](shared/src/log.rs), which
   uses `x86_64::instructions`) must be `#[cfg(target_arch = "x86_64")]`-
   gated to a no-op elsewhere, or the whole suite fails to compile on this
   host. **Fix that at the source** (cfg-gate) rather than working around
   it on the host (e.g. don't add a Rosetta `x86_64-apple-darwin` target
   just to run the tests) — a host workaround only fixes it on one
   machine and leaves the repo broken for everyone else. This
   root-cause-over-workaround preference applies generally: when host
   tooling can't run a repo command, first check whether the repo itself
   is wrong (missing cfg-gate, wrong target assumption) before reaching
   for an environment-side fix.
3. `cargo smiri` — same `shared` tests under Miri (`rustup component add
   miri` once). Miri interprets the harness's unsafe page-table pointer
   walks and flags out-of-bounds/use-after-free/uninit/provenance errors.
   Two gotchas, both already handled in `.cargo/config.toml`'s
   `MIRIFLAGS`:
   * **Permissive** (not strict) provenance is required — the paging code
     models physical addresses as integers and round-trips them through
     pointers (`VirtAddress` is a bare `u64` → `as_mut_ptr`), which strict
     provenance rejects outright. Don't switch it back without reworking
     that model.
   * Isolation is disabled so `proptest` can call `getcwd`; stacked
     borrows stay disabled for the intrusive-collection code.
4. Actually boot it and read the debugcon output (port 0xE9, see
   [shared/src/log.rs](shared/src/log.rs)).

### Booting headlessly and bounding the run

Rebuild first (`./make-image.sh`), then run headless and capture output.
QEMU never exits on its own after the kernel halts, and there's **no
`timeout`/`gtimeout` on macOS**. A `perl -e 'alarm N; exec ...'` wrapper
does **not** work either — QEMU installs its own `SIGALRM` handler and
ignores it. Bound it externally with a `pkill` watchdog instead:

```
./run-qemu.sh -debugcon stdio -display none >log 2>/dev/null &
SPID=$!
( sleep 45; pkill -KILL -f qemu-system-x86_64 2>/dev/null ) &
wait $SPID 2>/dev/null; cat log
```

**Budget > 40s for a full boot.** A run currently spends ~30-40s (under
QEMU TCG on Apple Silicon — no HVF/KVM since this is an x86_64 guest on an
arm64 host) between the `kernel loaded and mapped` and `identity mapped
existing memory` debugcon lines. That's **known issue #5** (the loader
identity-maps the entire UEFI memory map, including a multi-GiB reserved
region, at 4 KiB granularity) — not a hang or a regression. Don't mistake
it for one; only dig in if the boot never progresses past that point
within ~60s. A successful boot's debugcon currently progresses roughly:
`kernel loaded and mapped` → `identity mapped existing memory, exiting
boot services` → `Exited boot services` → `Installed page table` → the
`kernel_entry` line (which may change — verify against current
[src/kmain.rs](src/kmain.rs), since `kernel_entry` is WIP per "Project
status" above).

Prefer watching live over blindly blocking on the full budget: start QEMU
in the background (log redirected to a file as above), then poll the log
file for the expected success line or a failure signature (`panic`,
`Killed`) and kill QEMU as soon as one appears, rather than waiting out
the whole 45s every time. The `pkill` watchdog must still run underneath
as the fallback, since QEMU never exits on its own if the expected line
never shows up (a real hang).

If the log just stops with no panic line, that's the signature of a
triple fault (QEMU silently resets without `-no-reboot`). Add
`-no-reboot -no-shutdown -d int,cpu_reset -D <qemu-debug-log>` to get
QEMU's own exception/reset trace instead of re-guessing from debugcon
silence alone.

## Contribution conventions

* Keep commit messages concise (short subject, at most a brief sentence or
  two of body). Detailed rationale, investigation notes, and test plans
  belong in the PR description, not duplicated into the commit — a commit
  can always be traced back to its PR. This applies to squash-merge
  commits too (use the PR title as the subject, empty body is fine).
* "Submit the PR" / "ship this" / "land this" means the **full flow**:
  push the branch, open the PR, merge it (match the repo's existing merge
  convention — check a recent merged PR's commit graph, e.g.
  `git log --format='%H %P' -1 <merge-commit>`, to see whether it's
  squashed (single parent) or a real merge (two parents), and use the
  matching `gh pr merge` flag), switch back to `master`, and pull/
  fast-forward so local `master` actually has the merged changes. Also
  delete the local and remote feature branch afterward and
  `git fetch --prune`. If the phrasing is ambiguous about whether merging
  is wanted (e.g. just "push this up" or "open a PR"), stop at opening
  the PR and ask.
* When filing a GitHub issue or opening a PR, preface the body with a
  short note that the content is agent-generated, e.g.:
  > _Filed by an AI coding agent (Claude Code)._

  before the substantive content.
