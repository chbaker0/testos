//! Data structures representing pages and frames

use super::addr::{Length, PhysAddress, PhysExtent, VirtAddress, VirtExtent};

use core::iter;
use core::num::NonZeroU64;

pub const PAGE_SIZE: Length = Length::from_raw(4096);

/// A 4 KiB physical memory frame
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct Frame {
    start: PhysAddress,
}

impl Frame {
    /// Creates a `Frame` representing the frame beginning at `start`.
    ///
    /// # Panics
    ///
    /// Panics if `start` is not aligned to `PAGE_SIZE`.
    pub fn new(start: PhysAddress) -> Frame {
        assert!(start.is_aligned_to(PAGE_SIZE.as_raw()));
        Frame { start }
    }

    /// Which number frame this is; in other words, the start address divided by
    /// the page size.
    pub fn index(self) -> u64 {
        self.start.as_raw() / PAGE_SIZE.as_raw()
    }

    /// Gets the `Frame` that contains `addr`.
    pub fn containing(addr: PhysAddress) -> Frame {
        Self::new(addr.align_down(PAGE_SIZE.as_raw()))
    }

    /// Start address of the frame
    pub fn start(self) -> PhysAddress {
        self.start
    }

    /// Extent of memory contained in the frame
    pub fn extent(self) -> PhysExtent {
        PhysExtent::new(self.start, PAGE_SIZE)
    }

    /// The nth frame after `self`, or `None` if it's not addressable
    pub fn next(self, n: u64) -> Option<Frame> {
        let next_start = self
            .start
            .offset_by_checked(Length::from_raw(PAGE_SIZE.as_raw().checked_mul(n)?))?;
        Some(Self::new(next_start))
    }
}

/// A 4 KiB virtual memory page
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct Page {
    start: VirtAddress,
}

impl Page {
    /// Creates a `Page` representing the page beginning at `start`.
    ///
    /// # Panics
    ///
    /// Panics if `start` is not aligned to `PAGE_SIZE`.
    pub fn new(start: VirtAddress) -> Page {
        assert!(start.is_aligned_to(PAGE_SIZE.as_raw()));
        Page { start }
    }

    /// Gets the `Page` that contains `addr`.
    pub fn containing(addr: VirtAddress) -> Page {
        Self::new(addr.align_down(PAGE_SIZE.as_raw()))
    }

    /// Start address of the page
    pub fn start(&self) -> VirtAddress {
        self.start
    }

    /// Extent of virtual address space contained in the page
    pub fn extent(&self) -> VirtExtent {
        VirtExtent::new(self.start, PAGE_SIZE)
    }

    /// The nth page after `self`, or `None` if it's not addressable
    pub fn next(self, n: u64) -> Option<Page> {
        let next_start = self
            .start
            .offset_by_checked(Length::from_raw(PAGE_SIZE.as_raw().checked_mul(n)?))?;
        Some(Self::new(next_start))
    }

    pub fn l4_index(self) -> usize {
        const FIRST_BIT: u32 = 12 + 9 + 9 + 9;
        ((self.start.as_raw() & (0b1_1111_1111 << FIRST_BIT)) >> FIRST_BIT) as usize
    }

    pub fn l3_index(self) -> usize {
        const FIRST_BIT: u32 = 12 + 9 + 9;
        ((self.start.as_raw() & (0b1_1111_1111 << FIRST_BIT)) >> FIRST_BIT) as usize
    }

    pub fn l2_index(self) -> usize {
        const FIRST_BIT: u32 = 12 + 9;
        ((self.start.as_raw() & (0b1_1111_1111 << FIRST_BIT)) >> FIRST_BIT) as usize
    }

    pub fn l1_index(self) -> usize {
        const FIRST_BIT: u32 = 12;
        ((self.start.as_raw() & (0b1_1111_1111 << FIRST_BIT)) >> FIRST_BIT) as usize
    }
}

/// A contiguous range of physical memory frames. Always non-empty.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FrameRange {
    first: Frame,
    count: NonZeroU64,
}

impl FrameRange {
    pub fn new(first: Frame, count: u64) -> Option<FrameRange> {
        let count = NonZeroU64::new(count)?;

        // Check that `count` frames after and including `first` are
        // addressable. `first.next(count)` may not be addressable if the range
        // includes the last frame.
        first.next(count.get() - 1)?;

        Some(FrameRange { first, count })
    }

    // A single frame
    pub fn one(frame: Frame) -> FrameRange {
        Self::new(frame, 1).unwrap()
    }

    // All frames between and including `first` to `last`
    pub fn between_inclusive(first: Frame, last: Frame) -> FrameRange {
        let len = last.start() - first.start();
        let count = len.as_raw() / PAGE_SIZE.as_raw() + 1;
        Self::new(first, count).unwrap()
    }

    // All frames between `first` to `last`, including `first` but not `last`
    pub fn between_exclusive(first: Frame, last: Frame) -> FrameRange {
        let len = last.start() - first.start();
        let count = len.as_raw() / PAGE_SIZE.as_raw();
        Self::new(first, count).unwrap()
    }

    /// The minimal range fully containing `extent`.
    pub fn containing_extent(extent: PhysExtent) -> FrameRange {
        let first = Frame::containing(extent.address());
        let last = Frame::containing(extent.last_address());
        Self::between_inclusive(first, last)
    }

    /// The maximal range fully contained in `extent`.
    ///
    /// Returns `None` if no whole frame fits. That includes the case where
    /// `extent` starts inside the final partial page of the address space, so
    /// its start has no page-aligned successor: no aligned frame can fit above
    /// a start that isn't representable, so `None` is the right answer rather
    /// than a panic.
    pub fn contained_by_extent(extent: PhysExtent) -> Option<FrameRange> {
        let first = extent.address().align_up_checked(PAGE_SIZE.as_raw())?;
        let last = extent.end_address().align_down(PAGE_SIZE.as_raw());
        if first >= last {
            return None;
        }

        let len = last - first;
        assert!(len.is_aligned_to(PAGE_SIZE.as_raw()));
        FrameRange::new(Frame::new(first), len.as_raw() / PAGE_SIZE.as_raw())
    }

    pub fn first(&self) -> Frame {
        self.first
    }

    pub fn count(&self) -> u64 {
        self.count.get()
    }

    // The last `Frame` within the range
    pub fn last(&self) -> Frame {
        self.first.next(self.count.get() - 1).unwrap()
    }

    // The first `Frame` after the range, or `None` if it ends at the last frame.
    pub fn end(&self) -> Option<Frame> {
        self.first.next(self.count.get())
    }

    pub fn iter(&self) -> impl Clone + Iterator<Item = Frame> + use<> {
        let last = self.last();
        iter::successors(Some(self.first), move |frame| {
            if frame < &last {
                frame.next(1)
            } else {
                None
            }
        })
    }
}

/// A contiguous range of virtual memory pages. Always non-empty.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PageRange {
    first: Page,
    count: u64,
}

impl PageRange {
    pub fn new(first: Page, count: u64) -> Option<PageRange> {
        if count == 0 {
            return None;
        }

        // Check that `count` frames after and including `first` are
        // addressable. `first.next(count)` may not be addressable if the range
        // includes the last frame.
        first.next(count - 1)?;

        Some(PageRange { first, count })
    }

    // A single page
    pub fn one(page: Page) -> PageRange {
        Self::new(page, 1).unwrap()
    }

    // All frames between and including `first` to `last`
    pub fn between_inclusive(first: Page, last: Page) -> PageRange {
        let len = last.start() - first.start();
        let count = len.as_raw() / PAGE_SIZE.as_raw() + 1;
        Self::new(first, count).unwrap()
    }

    // All frames between `first` to `last`, including `first` but not `last`
    pub fn between_exclusive(first: Page, last: Page) -> Option<PageRange> {
        let len = last.start() - first.start();
        let count = len.as_raw() / PAGE_SIZE.as_raw();
        Self::new(first, count)
    }

    pub fn containing_extent(extent: VirtExtent) -> PageRange {
        let first = Page::containing(extent.address());
        let last = Page::containing(extent.last_address());
        Self::between_inclusive(first, last)
    }

    pub fn first(&self) -> Page {
        self.first
    }

    pub fn count(&self) -> u64 {
        self.count
    }

    // The last `Page` within the range
    pub fn last(&self) -> Page {
        self.first.next(self.count - 1).unwrap()
    }

    // The first `Page` after the range, or `None` if it ends at the last frame.
    pub fn end(&self) -> Option<Page> {
        self.first.next(self.count)
    }

    pub fn iter(&self) -> impl Iterator<Item = Page> + use<> {
        let last = self.last();
        iter::successors(Some(self.first), move |page| {
            if page < &last {
                page.next(1)
            } else {
                None
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// An extent starting inside the last partial page has no page-aligned
    /// start, so no whole frame fits in it. `contained_by_extent` used to
    /// panic aligning that start up; it must report `None` instead.
    #[test]
    fn contained_by_extent_reports_none_at_the_top_of_the_address_space() {
        // Starts at ...F001, one byte above the last page boundary, and runs to
        // the highest address `Extent`'s own invariant allows.
        let e = PhysExtent::from_raw(u64::MAX - 0xFFE, 0xFFE);
        assert_eq!(FrameRange::contained_by_extent(e), None);
    }
}

#[cfg(kani)]
mod verify {
    //! Kani proof harnesses for [`crate::memory::page`].
    //!
    //! The headline result here is
    //! [`page_indices_decompose_the_virtual_address`]: the four `l*_index`
    //! accessors are the *only* thing standing between a virtual address and the
    //! page-table slot `Mapper::map` writes into. A shift or mask off by one bit
    //! in any of them produces a mapping that silently lands on the wrong page —
    //! no panic, no assertion, just the wrong physical memory. The existing unit
    //! tests exercise them at a handful of hand-picked addresses; this proves the
    //! decomposition is a bijection over the whole 48-bit page-aligned domain.

    use super::*;

    /// A symbolic page-aligned virtual address — the only kind `Page::new`
    /// accepts.
    fn any_page_aligned_virt() -> VirtAddress {
        let raw: u64 = kani::any();
        kani::assume(raw & (PAGE_SIZE.as_raw() - 1) == 0);
        VirtAddress::from_raw(raw)
    }

    /// A symbolic frame-aligned physical address.
    fn any_frame_aligned_phys() -> PhysAddress {
        let raw: u64 = kani::any();
        kani::assume(raw & (PAGE_SIZE.as_raw() - 1) == 0);
        PhysAddress::from_raw(raw)
    }

    // ---------------------------------------------------------------------------
    // Page-table index decomposition
    // ---------------------------------------------------------------------------

    /// The four table indices partition bits 12..48 of a page-aligned virtual
    /// address, each exactly 9 bits wide, with no gap and no overlap.
    ///
    /// Proving *recomposition* (rather than just checking each index in isolation)
    /// is what rules out the whole class of bugs where two levels read overlapping
    /// bit ranges: if `l2_index` and `l1_index` shared a bit, two distinct pages
    /// would decompose to the same slot quadruple and the reassembled address
    /// would not match the original.
    #[kani::proof]
    fn page_indices_decompose_the_virtual_address() {
        let start = any_page_aligned_virt();
        let p = Page::new(start);

        let (l4, l3, l2, l1) = (p.l4_index(), p.l3_index(), p.l2_index(), p.l1_index());

        // Each index addresses one of the 512 slots in a `PageTable`. `map` uses
        // these to index `entries: [PageTableEntry; 512]` directly, so anything
        // >= 512 would be an out-of-bounds table access.
        assert!(l4 < 512 && l3 < 512 && l2 < 512 && l1 < 512);

        // Reassembling the four fields recovers exactly the translated portion of
        // the address (bits 12..48). Bits 48..64 are the sign extension x86-64
        // requires of a canonical address and are not part of the walk.
        let rebuilt =
            ((l4 as u64) << 39) | ((l3 as u64) << 30) | ((l2 as u64) << 21) | ((l1 as u64) << 12);
        assert_eq!(rebuilt, start.as_raw() & 0x0000_FFFF_FFFF_F000);
    }

    /// The contrapositive of the decomposition, stated the way the page-table walk
    /// depends on it: two pages within the same 48-bit half of the address space
    /// share all four indices only if they are the same page. If this failed, two
    /// distinct virtual pages would alias the same leaf slot and one mapping would
    /// silently clobber the other.
    #[kani::proof]
    fn distinct_pages_get_distinct_index_quadruples() {
        let a = Page::new(any_page_aligned_virt());
        let b = Page::new(any_page_aligned_virt());
        // Restrict to the low canonical half, where the address is fully
        // determined by its translated bits (the upper half's addresses differ
        // only in sign-extension bits the walk never looks at).
        kani::assume(a.start().as_raw() < 0x0001_0000_0000_0000);
        kani::assume(b.start().as_raw() < 0x0001_0000_0000_0000);

        let same_slots = a.l4_index() == b.l4_index()
            && a.l3_index() == b.l3_index()
            && a.l2_index() == b.l2_index()
            && a.l1_index() == b.l1_index();

        assert_eq!(same_slots, a == b);
    }

    // ---------------------------------------------------------------------------
    // Frame / Page basics
    // ---------------------------------------------------------------------------

    #[kani::proof]
    fn frame_containing_covers_its_address() {
        let addr = PhysAddress::from_raw(kani::any());
        let f = Frame::containing(addr);

        assert!(f.start().is_aligned_to_length(PAGE_SIZE));
        assert!(f.start() <= addr, "the frame starts at or below the address");
        assert!(
            addr.as_raw() - f.start().as_raw() < PAGE_SIZE.as_raw(),
            "and the address is within one page of the start"
        );
        // `containing` must be idempotent — a frame's own start maps back to it.
        assert_eq!(Frame::containing(f.start()), f);
    }

    #[kani::proof]
    fn frame_index_round_trips_through_start() {
        let f = Frame::new(any_frame_aligned_phys());

        assert_eq!(f.index() * PAGE_SIZE.as_raw(), f.start().as_raw());
        assert_eq!(f.extent().address(), f.start());
        assert_eq!(f.extent().length(), PAGE_SIZE);
    }

    /// `Frame::next` is the addressability guard the range types lean on: it must
    /// return `None` exactly when the target frame would not fit in the address
    /// space, and never wrap silently.
    #[kani::proof]
    fn frame_next_returns_none_exactly_on_overflow() {
        let f = Frame::new(any_frame_aligned_phys());
        let n: u64 = kani::any();

        let representable = n
            .checked_mul(PAGE_SIZE.as_raw())
            .and_then(|off| f.start().as_raw().checked_add(off));

        match f.next(n) {
            Some(next) => {
                assert_eq!(next.start().as_raw(), representable.unwrap());
                assert!(next >= f, "advancing never moves backwards");
            }
            None => assert!(representable.is_none()),
        }
    }

    // ---------------------------------------------------------------------------
    // FrameRange
    //
    // `iter_map_frames` -> `FrameRange::containing_extent` is how every UEFI
    // memory-map entry becomes a set of frames the allocator may hand out. A range
    // that reached one frame beyond its extent would mark memory free that the
    // firmware never said was RAM.
    // ---------------------------------------------------------------------------

    #[kani::proof]
    fn frame_range_new_upholds_its_invariants() {
        let first = Frame::new(any_frame_aligned_phys());
        let count: u64 = kani::any();

        match FrameRange::new(first, count) {
            Some(r) => {
                assert!(count != 0, "ranges are documented as always non-empty");
                assert_eq!(r.count(), count);
                assert_eq!(r.first(), first);
                // `last()` unwraps internally; proving it total here is what makes
                // that unwrap safe for every constructible range.
                assert_eq!(r.last().index(), first.index() + count - 1);
            }
            None => assert!(count == 0 || first.next(count - 1).is_none()),
        }
    }

    /// `containing_extent` is documented as "the minimal range fully containing
    /// `extent`". Proved at a symbolic probe address: the range covers every byte
    /// of the extent (soundness) and both of its end frames actually touch the
    /// extent (minimality — it doesn't over-reach into neighbouring memory).
    #[kani::proof]
    fn frame_range_containing_extent_is_minimal_cover() {
        let address: u64 = kani::any();
        let length: u64 = kani::any();
        kani::assume(length != 0);
        kani::assume(length <= u64::MAX - address);
        let e = PhysExtent::new(PhysAddress::from_raw(address), Length::from_raw(length));

        let r = FrameRange::containing_extent(e);

        // Soundness: every address in the extent falls inside the frame range.
        let first_byte = r.first().start().as_raw();
        let last_byte = r.last().start().as_raw() + PAGE_SIZE.as_raw() - 1;
        assert!(first_byte <= address);
        assert!(last_byte >= e.last_address().as_raw());

        // Minimality: dropping either end frame would uncover part of the extent.
        assert!(Frame::containing(e.address()) == r.first());
        assert!(Frame::containing(e.last_address()) == r.last());
    }

    /// The dual: `contained_by_extent` is "the maximal range fully contained in
    /// `extent`", used wherever we must not round *outward* past what firmware
    /// reported as usable.
    #[kani::proof]
    fn frame_range_contained_by_extent_never_escapes() {
        let address: u64 = kani::any();
        let length: u64 = kani::any();
        kani::assume(length != 0);
        // `contained_by_extent` calls `end_address()`, which is exclusive and so
        // is unrepresentable for an extent covering the very last byte. That is
        // a genuine precondition of `Extent` itself, not of this function, so it
        // stays assumed away. Nothing else is: in particular the extent may
        // *start* inside the final partial page, which is the case that used to
        // panic in `align_up`.
        kani::assume(length <= u64::MAX - address);
        let e = PhysExtent::new(PhysAddress::from_raw(address), Length::from_raw(length));

        if let Some(r) = FrameRange::contained_by_extent(e) {
            let first_byte = r.first().start().as_raw();
            let last_byte = r.last().start().as_raw() + PAGE_SIZE.as_raw() - 1;

            assert!(first_byte >= address, "range starts inside the extent");
            assert!(
                last_byte <= e.last_address().as_raw(),
                "range ends inside the extent"
            );
            assert!(r.first().start().is_aligned_to_length(PAGE_SIZE));
        }
    }

    /// `end()` (one past the range) and `last()` (the final frame) must stay
    /// consistent, since `BumpFrameAllocator` and `mm::init` both do range
    /// arithmetic across the two.
    #[kani::proof]
    fn frame_range_end_is_one_past_last() {
        let first = Frame::new(any_frame_aligned_phys());
        let count: u64 = kani::any();
        kani::assume(count != 0);
        kani::assume(count <= 1024); // keeps the proof about arithmetic, not size
        kani::assume(first.next(count - 1).is_some());

        let r = FrameRange::new(first, count).unwrap();

        match r.end() {
            Some(end) => assert_eq!(end.index(), r.last().index() + 1),
            // Only a range butting against the very top of the address space has
            // no representable end frame.
            None => assert!(r.last().next(1).is_none()),
        }
    }

    // ---------------------------------------------------------------------------
    // PageRange
    // ---------------------------------------------------------------------------

    #[kani::proof]
    fn page_range_new_upholds_its_invariants() {
        let first = Page::new(any_page_aligned_virt());
        let count: u64 = kani::any();

        match PageRange::new(first, count) {
            Some(r) => {
                assert!(count != 0);
                assert_eq!(r.count(), count);
                assert_eq!(r.last().start().as_raw(), first.start().as_raw() + (count - 1) * PAGE_SIZE.as_raw());
            }
            None => assert!(count == 0 || first.next(count - 1).is_none()),
        }
    }

    /// `between_inclusive` divides by `PAGE_SIZE` and adds one; prove the count is
    /// exactly the number of pages spanned, and note the unstated precondition
    /// that `first <= last` (otherwise the `Sub` impl's `checked_sub` unwraps on a
    /// negative span).
    #[kani::proof]
    fn page_range_between_inclusive_counts_correctly() {
        let first = Page::new(any_page_aligned_virt());
        let last = Page::new(any_page_aligned_virt());
        kani::assume(first <= last);
        kani::assume(last.start().as_raw() - first.start().as_raw() <= 1024 * PAGE_SIZE.as_raw());

        let r = PageRange::between_inclusive(first, last);

        assert_eq!(r.first(), first);
        assert_eq!(r.last(), last);
        assert_eq!(
            r.count(),
            (last.start().as_raw() - first.start().as_raw()) / PAGE_SIZE.as_raw() + 1
        );
    }
}
