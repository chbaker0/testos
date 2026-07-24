# Verification with Kani

[Kani](https://model-checking.github.io/kani/) is a bounded model checker for
Rust. Where `cargo stest` samples inputs and `cargo smiri` watches one concrete
execution for undefined behaviour, Kani *proves* a property over every input in
a symbolic domain — or produces a concrete counterexample.

Each module under proof carries a `#[cfg(kani)] mod verify` block at the
bottom of its own file, in the same place and style as its `#[cfg(test)] mod
tests`. Being a child of the module under proof is what lets a harness reach
private items — the `TableStore` trait, `align_u64_down`, `find_bit_group`,
`PageTableEntry::raw`, `BlockAdapter` — which is where most of the interesting
invariants actually live.

```bash
cargo skani                                  # every harness
cargo kani -p shared --harness <name>        # one harness
cargo kani -p shared --output-format terse   # one line per harness
```

Kani installs its own toolchain and CBMC backend; `cargo install --locked
kani-verifier && cargo kani setup` is a one-time step (~1 GB).

## What each tool is for

The three checkers overlap deliberately little:

| | covers | misses |
|---|---|---|
| `cargo stest` | end-to-end behaviour, `proptest` sampling | rare inputs; anything not sampled |
| `cargo smiri` | undefined behaviour in the **unsafe** pointer walks (`PhysTableStore`) | only the paths the tests execute |
| `cargo skani` | **all** inputs to the safe logic — arithmetic, bit packing, traversal | raw-pointer UB, concurrency, unbounded loops |

Kani and Miri are complements on `paging.rs` specifically: Miri drives
`PhysTableStore`'s real `read_volatile`/`write_volatile` walks and checks them
for UB; Kani drives the same `Mapper::map` traversal through a pointer-free
array-backed store and proves it computes the *right answer* for every page.
Neither subsumes the other.

## How the harnesses are written

Three conventions run through the suite.

**Prove against a specification, not against the implementation.** Two forms
show up. Single-extent operations use a *probe address*: a symbolic `p`, with
the assertion `contains_addr(result, p) == spec(p)`. Because `p` is
universally quantified, that is extensional equality of two sets — it says the
operation computes the right range, not merely that it agrees with itself.
Two-extent operations (`overlap`, `contains`, the differences) instead name
their answer's *endpoints* directly, e.g. `overlap` starts at
`max(a.start, b.start)` and ends at `min(a.last, b.last)`. For intervals the
two forms are equivalent, but the endpoint form is sharper and much cheaper:
a probe adds a third symbolic 64-bit value on top of two extents' four, which
CBMC does not settle in useful time.

`Mapper::map` gets an oracle rather than a formula: an independent `translate`
walks the tables the way hardware does — stopping early at a `PAGE_SIZE` leaf
and reapplying the offset — and shares no code with `Mapper`.

The same discipline applies inside a harness: an assertion should not call the
function under proof. `shrink_to_alignment`'s result is checked against raw
endpoints rather than `e.contains(s)`, because `contains` is itself under
proof a few harnesses up.

**An `assume` is a precondition, written down.** Where a harness narrows its
input domain, that narrowing *is* the function's contract. If the contract
isn't in the doc comment, the harness comment says so and links the finding.
This is how the suite turned up `shrink_to_alignment`'s unchecked
`align_u64_up` — the harness failed until the real precondition was stated.

**`#[kani::should_panic]` documents the other side of a contract.** A harness
that asserts a function *always* panics on out-of-domain input pins the exact
boundary — `Length::num_pages` at zero, `PageTableEntry::set_addr` at 2^52 and
on misalignment, `BitmapFrameAllocator::deallocate` on a double free.

## Scope and bounds

Kani is *bounded*: loops need an unwind limit, and some domains have to be
narrowed to keep the solver tractable. Every such reduction in this suite is
deliberate and commented at the point it's made. The two recurring ones:

* **Alignments.** The primitive `align_u64_down`/`align_u64_up` proofs quantify
  over all 64 power-of-two alignments. The compound `Extent` proofs enumerate
  only 4 KiB / 2 MiB / 1 GiB — every value any real call site passes — because
  a symbolic shift amount combined with `align_up` + `align_down` +
  `overlap`'s comparison chain pushes CBMC past useful runtimes.

* **`map_range`.** Its five phases can't be executed over a realistic extent
  (thousands of iterations). Instead each harness pins the extent's *alignment*
  so that only the phases under test can run, and bounds the length so they run
  a handful of times — then checks the property that matters end to end: a
  symbolic probe address translates if and only if it is inside the extent, and
  to the right physical address. Over-mapping past `end` is checked explicitly,
  since that failure mode is silent.

Addresses, frames, flags, bitmaps and entry contents stay fully symbolic
throughout. Those are the operands where a bug would hide.

## Findings

Proofs that currently fail — each because it found a real defect — are
catalogued in [kani-findings.md](kani-findings.md), with the counterexample and
the reachability analysis for each. A harness that is expected to fail says so
in its own doc comment too, so the source and the catalogue can't drift.
