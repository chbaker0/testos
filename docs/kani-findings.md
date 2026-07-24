# Kani findings

Defects surfaced by the proof harnesses in the `#[cfg(kani)] mod verify` block
of each module under proof. See [verification.md](verification.md) for how to
run them.

Each entry records the counterexample, why it is or isn't reachable from real
call sites, and what was done about it. "Reachable" matters: a bounded model
checker quantifies over the whole input domain, so it finds contract gaps that
no current caller can trigger alongside ones that bite today. Both are worth
writing down; only the first kind is worth losing sleep over.

---

## Fixed

### `shrink_to_alignment` panicked near the top of the address space

**Harness:** `memory::addr::verify::shrink_to_alignment_stays_inside_and_is_aligned`
**Site:** [`shared/src/memory/addr.rs`](../shared/src/memory/addr.rs) —
`align_u64_up`, reached via `Extent::shrink_to_alignment`

`align_u64_up` computed `x + (alignment - 1)` unchecked. For any extent
starting inside the last partial aligned block of the address space — with
4 KiB alignment, a start above `0xFFFF_FFFF_FFFF_F000` — that addition
overflows.

```
Failed Checks: attempt to add with overflow
 File: "shared/src/memory/addr.rs", line 404, in memory::addr::align_u64_up
```

Two failure modes, and the quieter one is worse. With overflow checks on
(the workspace's dev profile) it panics. With them off, it *wraps* to a small
value, so `shrink_to_alignment` would return an extent near address 0 that has
nothing to do with its input — and `iter_map_frames` would hand those frames
to the allocator as free RAM.

Not reachable in practice today: the only in-tree caller is `iter_map_frames`,
at 4 KiB alignment, which would need a UEFI memory-map entry beginning in the
final 4 KiB of the 64-bit address space. But `shrink_to_alignment` is public
and generic over alignment, and nothing documented the restriction.

**Fix.** Added `align_u64_up_checked` returning `Option<u64>`, plus
`Address::align_up_checked`. `shrink_to_alignment` now propagates `None`, which
is the right answer: if no aligned start is representable, no aligned
sub-extent exists, since the extent's end can only be lower still. The
panicking `align_u64_up` is now a thin wrapper over the checked form, so a
release build faults loudly instead of wrapping silently.

Proving the fix also collapsed the harness from >4 minutes to ~1 second —
`checked_add` gives the solver a far simpler query than the wrapping add did.

### `next_level` could clear `PRESENT` and detach a subtree

**Harness:** `memory::paging::verify::next_level_reuses_present_entries_and_allocates_otherwise`
**Site:** [`shared/src/memory/paging.rs`](../shared/src/memory/paging.rs) — `Mapper::next_level`

```
Status: FAILURE
Description: "assertion failed: entry.get_flags().contains(PageTableFlags::PRESENT)"
```

On the *reuse* branch (parent entry already present), flags were rewritten
wholesale as `flags & mask_flags | set_flags`. A caller passing a
`parent_mask_flags` without `PRESENT` and a `parent_set_flags` without it
leaves the entry still pointing at a real page table but marked **not
present** — while `map` carries on writing the rest of the path and the leaf
into that now-unreachable subtree, and returns `Ok(())`.

The result is a mapping that silently does not exist. Worse, the *next* `map`
through the same slot takes the not-present branch and allocates a fresh
table, leaking the old one — exactly the frame-ownership hazard the `NOTE:`
comment in `next_level` warns about.

Note the asymmetry that made this a bug rather than a design choice: the
*allocate* branch already forced `PRESENT` via
`set_flags.union(PageTableFlags::PRESENT)`. Only the reuse branch could
produce a non-present parent.

Not reachable from either in-tree caller — `mm::extend_page_table_with_physical_map`
and the loader both pass `PageTableFlags::all()` as the mask — so this was a
latent contract gap, and precisely the kind that a future caller wanting to
narrow parent permissions would fall into.

**Fix.** The reuse branch now re-applies `PRESENT` after masking, matching the
allocate branch. `PRESENT` is exempt from `parent_mask_flags`, documented on
both `next_level` and `Mapper::map`. The harness leaves `set`/`mask` fully
symbolic and needs no precondition: the property holds for every combination.

### `fill_bitmap_from_map` mis-marked small regions (two defects)

**Harnesses:** `memory::alloc::phys::verify::mark_frames_free_marks_exactly_its_frames`,
`…::fill_bitmap_marks_exactly_the_available_frames`
**Site:** [`shared/src/memory/alloc/phys.rs`](../shared/src/memory/alloc/phys.rs)

`fill_bitmap_from_map` marked an `Available` frame range into the allocator
bitmap in three phases — a leading partial byte, a run of whole bytes, a
trailing partial byte. Both partial-byte phases were wrong for a range that
does not span a byte boundary.

**Defect 1 — underflow.** The trailing phase asserted
`last_byte == (end_aligned - 1) / FRAMES_PER_ENTRY + 1`. When the range ends
below frame 8, `end_aligned` is `0` and the `- 1` underflows:

```
attempt to subtract with overflow, shared/src/memory/alloc/phys.rs:334
```

**Defect 2 — over-marking, and this is the dangerous one.** With the underflow
masked, a region covering only frames 1–3 produced `0b1111_1111`: the *whole*
byte marked free. The leading phase widened the range up to the next 8-frame
boundary and the trailing phase widened it down to the previous one, and the
two ORed together to cover everything. `mm::init` hands the result to
`BitmapFrameAllocator::new`, whose `unsafe` contract is "all frames marked free
must be available for use and not used by other code" — so those extra bits are
firmware memory, kernel image, or MMIO that the allocator will hand out as
ordinary RAM.

**Both are reachable at boot.** `mm::init` runs `fill_bitmap_from_map` over the
map the loader builds from UEFI; any `Available` region ending below 32 KiB
trips defect 1, and any region confined within a single bitmap byte trips
defect 2. That we have not hit them says only that current QEMU/OVMF maps
happen not to produce such a region.

**Fix.** Extracted `mark_frames_free(bitmap, frames)` and replaced the
three-phase split with a uniform per-byte loop that clips the range to each
byte it touches (`lo = max(first, byte_start)`, `hi = min(end, byte_end)`) and
ORs in `set_bit_range(lo, hi)`. The uniform form cannot express either defect:
there is no separate partial-byte path to get wrong, and no subtraction that
can underflow. A whole byte still costs one store, since `lo == 0 && hi == 8`
yields `u8::MAX`. `set_most_significant_bits` became dead and was replaced by
`set_bit_range`.

Extracting the function is also what made it provable: the harness now takes a
`FrameRange` directly, with no `Map` in the picture, and starts from a
*symbolic* bitmap — so it proves both that every frame in the range ends up
free and that every bit outside it is untouched.

### `contained_by_extent` panicked on an extent in the last partial page

**Harness:** `memory::page::verify::frame_range_contained_by_extent_never_escapes`
**Site:** [`shared/src/memory/page.rs`](../shared/src/memory/page.rs) —
`FrameRange::contained_by_extent`

The same `align_u64_up` overflow as the first finding, reached by a second
route — and initially *hidden* by the harness rather than caught by it. The
harness assumed `address + length <= u64::MAX - (PAGE_SIZE - 1)`, on the stated
grounds that `contained_by_extent` aligns up its `end_address()`. It doesn't: it
aligns *down* the end and aligns *up* the start. So the assumption excluded
every extent whose start sits in the final 4 KiB of the address space, which is
precisely the input that panics. Caught in review by Codex on PR #47.

```
PhysExtent::from_raw(u64::MAX - 0xFFE, 0xFFE)   // starts at ...F001
        -> panic in align_u64_up
```

**Fix.** `contained_by_extent` now uses `align_up_checked` and propagates
`None`, the same resolution as `shrink_to_alignment` — if the start has no
page-aligned successor, no whole frame fits above it. The harness keeps only
the `end_address()` precondition, which is a genuine constraint of `Extent`
itself (`new_checked` rejects a length reaching the last byte, so an extent
covering `u64::MAX` is unrepresentable). A regression unit test pins the case;
it panics against the old implementation.

The general lesson is worth recording, since it applies to every harness here:
**an `assume` that is wrong is worse than no proof at all**, because it produces
a green result over a domain nobody checked. This one was wrong for a reason
that reads plausibly in the comment and takes reading the callee to falsify.

### `Map::iter_type` scanned the dummy tail

**Harness:** `memory::verify::iter_type_matches_filtering_the_real_entries`
**Site:** [`shared/src/memory.rs`](../shared/src/memory.rs) — `Map::iter_type`

`iter_type` filtered `self.entries` — the whole 128-slot backing array —
rather than `self.entries()`, the `num_entries` prefix. `from_entries` fills
the unused tail with dummy `Reserved` entries at address 0, so asking for
`Reserved` returned up to 128 phantom one-byte extents. Invisible today only
because the sole caller asks for `Available`.

**Fix.** Filter `self.entries()`.

This also had a cost consequence worth recording: because every caller walked
128 slots, the end-to-end `fill_bitmap` harness had to unwind two 128-iteration
loops and would not settle in ten minutes. With the fix it verifies in 5
seconds. A representation bug and a verification-tractability problem turned
out to be the same bug.

---

## Open

### `find_bit_group`'s mask is 3 bits wide for a 4-frame run

**Harnesses:** `memory::alloc::phys::verify::find_bit_group_returns_a_fully_free_run`,
`…::find_bit_group_does_not_miss_an_available_run`,
`…::allocate_range_never_hands_out_a_used_frame`
**Site:** [`shared/src/memory/alloc/phys.rs`](../shared/src/memory/alloc/phys.rs) — `find_bit_group`

`find_bit_group(byte, len)` claims to find `len` consecutive *set* bits,
aligned to `len`. It builds its comparison mask as:

```rust
let mask = ((len << 1) - 1) as u8;
```

That is `2 * len - 1`, not `2^len - 1`. It coincides with the intended value
for `len` 1 (`1`) and `len` 2 (`3`), and diverges at `len` 4: the mask is
`0b111`, three bits, used to test a four-bit run. `len` is asserted `< 8` and a
power of two, so 1, 2, 4 is the entire domain — exactly one third of it is
wrong.

Concrete counterexample, extracted by probing the reported failure:

```
find_bit_group(0b0111_0111, 4) = Some(0)      // expected None: bit 3 is clear
```

The existing unit tests pass only by accident — every `len == 4` case they try
(`0b00001111`, `0b11110000`, `0b11101110`) happens to have its fourth bit agree
with the other three.

**Consequence: a physical frame handed out twice.** `allocate_range` trusts the
offset and clears a full `2^order`-bit mask at it:

```
allocate_range(2) on 0b0111_0111 -> frames 0..=3, bitmap now 0b0111_0000
        frame 0 was FREE
        frame 1 was FREE
        frame 2 was FREE
        frame 3 was *** ALREADY ALLOCATED ***
```

`FrameAllocator` is an `unsafe trait` whose second documented invariant is
"allocations do not return allocated or reserved frames". This breaks it: the
returned `FrameRange` contains a frame some earlier allocation still owns, so
two owners alias the same physical page. Every `SAFETY` comment in `mm.rs`
resting on "frames not in use anywhere else" rests on this.

Reachable through `FrameAllocator::allocate_range(2)`, and `HeapProvider`
computes its order from a requested chunk count, so a 4-chunk heap request
reaches it. `allocate()` uses order 0 and is unaffected — which is why nothing
has gone wrong yet.

The fix is `(1usize << len) - 1`.

### `allocate_range` panics instead of reporting exhaustion

**Harnesses:** `memory::alloc::phys::verify::allocate_range_one_byte_never_hands_out_a_used_frame`,
`…::allocate_range_two_bytes_never_hands_out_a_used_frame`
**Site:** [`shared/src/memory/alloc/phys.rs`](../shared/src/memory/alloc/phys.rs) — `allocate_range`, the `size >= 8` path

```
Failed Checks: internal error: entered unreachable code
 File: "shared/src/memory/alloc/phys.rs", line 231, in ...allocate_range
```

The whole-byte path scans aligned chunks and returns `None` only from *inside*
the loop, when a trailing chunk would run past the end of the bitmap. When the
bitmap length is an exact multiple of the chunk length, that early return never
fires, the loop simply ends, and control reaches `unreachable!()`.

```
allocate_range(3) on [0b0111_0111]  ->  panic at phys.rs:231
```

So "no run of 8 free frames is available" — an ordinary, expected outcome that
the `Option` return type exists to express — panics the kernel instead. Any
bitmap whose length divides evenly by the chunk size reaches it, which includes
the common case of a power-of-two-sized bitmap.

The fix is to return `None` after the loop rather than `unreachable!()`.


### `Extent` construction can bypass its own non-empty invariant

**Harness:** `memory::addr::verify::from_range_exclusive_can_violate_the_non_empty_invariant`
**Site:** [`shared/src/memory/addr.rs`](../shared/src/memory/addr.rs) —
`Extent::from_range_exclusive`, `Extent::from_range_inclusive`

`Extent::new_checked` rejects zero-length extents, so the type's constructor
treats "empty" as unrepresentable. `from_range_exclusive` builds the struct
literally instead, so `from_raw_range_exclusive(x, x)` yields a zero-length
`Extent` that `new_checked` would have refused. Every accessor assuming the
invariant then misbehaves — `last_address()` underflows on it.

Not fixed here because it is a semantic decision, not a mechanical one: making
these constructors return `Option` changes a widely-used API (including the
`const` call sites in `mm::VirtualMap`), and it is worth deciding deliberately
whether an empty extent should be representable at all.
