---
name: qemu-boot
description: Rebuild testos and boot it headlessly in QEMU to verify a change, bounding the run since QEMU never exits on its own and macOS has no timeout/gtimeout. Use whenever a testos change needs boot verification, or the user asks to boot/run/test the kernel, loader, or init in QEMU. Distinguishes a real hang from a silent triple-fault reset from a normal panic.
---

You are verifying a change to the testos kernel by actually booting it in QEMU
headlessly and reading the debugcon output, per the procedure in AGENTS.md.
Do not just run `./run-qemu.sh` in the foreground and block — it never exits
on its own once the kernel halts, and there is no `timeout`/`gtimeout` on this
macOS host, so you must bound the run externally.

## Procedure

1. **Confirm `.env` exists.** `make-image.sh` and `run-qemu.sh` both
   `source .env` and hard-fail if it's missing. If absent, copy
   `.env.example` to `.env` (empty is fine) before proceeding.

2. **Rebuild.** Run `./make-image.sh` in the repo root. This fetches OVMF
   prebuilts (first run only) and builds the loader + kernel into `out/esp`.
   Treat a failure here as a build error, not a boot error — report it and
   stop; don't proceed to boot a stale image.

3. **Determine the expected success signature before booting.** Read the
   current [src/kmain.rs](../../../src/kmain.rs) to see what `kernel_entry`
   and `kernel_main` currently do — this is still actively evolving, so
   don't assume last session's set of wired-up subsystems still matches.
   The expected sequence is: `kernel loaded and mapped` → `identity mapped
   existing memory, exiting boot services` → `Exited boot services` →
   `Installed page table` → `In kernel_entry` → GDT/IDT/frame-allocator
   lines → `In kernel_main` → whatever `kernel_main` currently does before
   halting.

4. **Boot in the background with an external watchdog.** QEMU installs its
   own `SIGALRM` handler, so a `perl -e 'alarm N; exec ...'` wrapper does
   *not* bound it — use a `pkill` watchdog instead:

   ```
   ./run-qemu.sh -debugcon stdio -display none >qemu.log 2>/dev/null &
   SPID=$!
   ( sleep 60; pkill -KILL -f qemu-system-x86_64 2>/dev/null ) &
   wait $SPID 2>/dev/null
   ```

   Default budget is 60s, as a hang backstop — a healthy boot now reaches
   `kernel_main` in well under 10s (the old ~30-40s stall between `kernel
   loaded and mapped` and `identity mapped existing memory`, issue #5, was
   fixed in `58fdf00`). Don't expect to need the full budget; only raise it
   if you have concrete reason to think the image got slower.

5. **Poll `qemu.log` live rather than blocking for the full budget.** Check
   every few seconds for either the current expected final success line
   (from step 3) or a failure signature (`panic`, or the log file suddenly
   stopping growing). As soon as one appears, kill QEMU
   (`pkill -KILL -f qemu-system-x86_64`) instead of waiting out the rest of
   the budget. The watchdog from step 4 remains as the fallback for a
   genuine hang (expected line never appears at all).

6. **Interpret the outcome:**
   - **Success line reached** → boot verified, report the tail of the log.
   - **`panic` in the log** → real panic, report the panic message and
     surrounding context; this is a code bug, not an environment issue.
   - **Log stops growing with no panic, before the expected line** → this
     is the signature of a silent triple fault (QEMU resets without
     `-no-reboot`). Re-run once with extra diagnostics to get a real trace
     instead of re-guessing from debugcon silence:

     ```
     ./run-qemu.sh -debugcon stdio -display none \
       -no-reboot -no-shutdown -d int,cpu_reset -D qemu-debug.log >qemu.log 2>/dev/null &
     ```

     apply the same background/watchdog/poll pattern, then report the
     exception/reset trace from `qemu-debug.log` (last exception before
     the reset is usually the actionable line).
   - **Stuck past budget with no panic and no progress at all** → real
     hang; don't assume it's issue #5, that's already fixed.

7. **Report** boot outcome, the relevant debugcon tail (and exception
   trace if a triple fault), and whether this matches or diverges from the
   expected sequence in step 3.

## Notes

- AGENTS.md's "Project status" section and the "Booting headlessly"
  section are the source of truth for current boot behavior/timing — but
  they're written by hand and can still drift out of date as `src/kmain.rs`
  evolves. If what you observe diverges from what those sections describe,
  say so and suggest updating AGENTS.md rather than silently trusting
  either the doc or your memory of a past run.
- A successful boot only verifies what actually executes at boot time —
  re-check `src/kmain.rs` each time rather than assuming last session's
  set of wired-up subsystems still matches.
- If you need interactive debugging rather than pass/fail verification,
  that's a different workflow (`./run-qemu.sh -s -S` + GDB) — out of scope
  for this skill.
- Always clean up: make sure no stray `qemu-system-x86_64` process is left
  running, and delete any log files you created in the repo root, after
  you're done, success or failure.
