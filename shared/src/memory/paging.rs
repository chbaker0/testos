use crate::memory::{addr::*, page::*};

use core::ptr;
use core::sync::atomic::{compiler_fence, Ordering};

use static_assertions as sa;

pub const MAX_PHYS_ADDR_BITS: u32 = 52;
pub const MAX_PHYS_ADDR: PhysAddress = PhysAddress::from_raw(1 << MAX_PHYS_ADDR_BITS);

#[derive(Clone, Debug)]
#[repr(C, align(4096))]
pub struct PageTable {
    entries: [PageTableEntry; 512],
}

impl PageTable {
    #[inline]
    /// Create a table where all entries are zero.
    pub const fn zero() -> PageTable {
        PageTable {
            entries: [PageTableEntry::zero(); 512],
        }
    }

    pub const fn entries(&self) -> &[PageTableEntry] {
        &self.entries
    }
}

// Assert that `PageTable` is 4 KiB.
sa::assert_eq_size!(PageTable, [u8; 4096]);

// The address field's bit position is declared once here and the shift/mask
// arithmetic is generated, rather than hand-written (see PR #10, PR #11,
// both hand-packing bugs in this exact field). `_low`/`_high` are padding
// that gives `pfn` its correct position; they alias `PageTableFlags`' bits
// and are never read/written through `AddrField` itself — `PageTableEntry`
// composes this with `PageTableFlags` to cover the full `u64`.
//
// `PageTableFlags` (below) is left as a `bitflags` type rather than folded
// into this bitfield: it's already sound (no shift/mask arithmetic to get
// wrong) and used at ~19 call sites across `mm.rs`/`loader` via bitwise
// combination (`PRESENT | WRITABLE`), which `bitflags` supports directly and
// a generated bitfield struct would not.
#[bitfield_struct::bitfield(u64)]
struct AddrField {
    #[bits(12)]
    _low: u64,
    #[bits(40)]
    pfn: u64,
    #[bits(12)]
    _high: u64,
}

#[derive(Clone, Copy, Debug)]
#[repr(transparent)]
pub struct PageTableEntry {
    raw: u64,
}

impl PageTableEntry {
    /// Create an entry with all bits set to zero.
    #[inline]
    pub const fn zero() -> PageTableEntry {
        PageTableEntry { raw: 0 }
    }

    /// Set the entry's physical address. For L1 entries this is the memory
    /// frame being mapped to. For L2+, this is the address of a lower-level
    /// table.
    ///
    /// # Panics
    /// Panics if `addr` is not aligned to a 4KiB boundary. Note that this
    /// doesn't guarantee safety: if using 2 MiB or 1 GiB pages, the address
    /// must be aligned likewise.
    ///
    /// Panics if `addr` exceeds 2^52, which is the upper bound on supported
    /// physical addresses. Does not check the CPU-specific maximum.
    #[inline]
    pub fn set_addr(&mut self, addr: PhysAddress) {
        assert!(addr.is_aligned_to_length(PAGE_SIZE), "{addr:?}");
        assert!(addr < MAX_PHYS_ADDR);
        // Re-derive `AddrField` from the current raw bits, overwrite only the
        // `pfn` field (the generated setter handles the shift/mask), and write
        // back. `_low`/`_high` — i.e. the flag bits — round-trip untouched, so
        // this replaces the address without disturbing flags and without
        // OR-accumulating across repeated calls.
        let mut fields = AddrField::from(self.raw);
        fields.set_pfn(addr.as_raw() >> 12);
        self.raw = fields.into();
    }

    #[inline]
    pub fn get_addr(&self) -> PhysAddress {
        PhysAddress::from_raw(AddrField::from(self.raw).pfn() << 12)
    }

    /// Set flags (as documented in `PageTableFlags`). Flag bits not present in
    /// `flags` are cleared; the address bits are left untouched. This assigns
    /// rather than OR-accumulates, so callers can rely on it to actually clear
    /// previously-set flags.
    #[inline]
    pub fn set_flags(&mut self, flags: PageTableFlags) {
        self.raw = (self.raw & !PageTableFlags::all().bits()) | flags.bits();
    }

    /// Get flags (as documented in `PageTableFlags`).
    #[inline]
    pub fn get_flags(&self) -> PageTableFlags {
        // SAFETY: PageTableFlags::all().bits() only returns bits valid for
        // PageTableFlags. Bitwise-and with any other value will yield only
        // valid bits.
        PageTableFlags::from_bits(self.raw & PageTableFlags::all().bits()).unwrap()
    }

    pub const fn as_raw(&self) -> u64 {
        self.raw
    }
}

// The frame address occupies bits 12..=51 of an entry (up to the 52-bit
// physical address maximum). Bits 0..=11 are zero by 4 KiB alignment; bits
// 52..=63 are reserved or hold flags.
pub const PAGE_TABLE_ENTRY_ADDR_BITS: u64 = ((1 << 40) - 1) << 12;

bitflags::bitflags! {
    /// Control bits for a page table entry. Documented in architecture manual.
    /// Note that some bits may not be valid for some table levels, and not
    /// every combination of bits may be valid.
    ///
    /// Entries prefixed with `APP_` are from "available" bits, so any meaning
    /// is attributed by us.
    #[derive(Clone, Copy, Debug)]
    pub struct PageTableFlags: u64 {
        const PRESENT = 1 << 0;
        const WRITABLE = 1 << 1;
        const USER = 1 << 2;
        const WRITE_THROUGH = 1 << 3;
        const NO_CACHE = 1 << 4;
        const ACCESSED = 1 << 5;
        const DIRTY = 1 << 6;
        const PAGE_SIZE = 1 << 7;
        const GLOBAL = 1 << 8;
        const EXECUTE_DISABLE = 1 << 63;

        /// A non-leaf entry with this bit is "frozen", meaning all descendent
        /// tables cannot be modified. This allows for mappings shared by
        /// multiple address spaces; remapping one should not change any others.
        ///
        /// Kernel mappings shared between all processes have this and the
        /// `GLOBAL` bit set.
        const APP_PARENT_FROZEN = 1 << 62;

        const DEFAULT_PARENT_TABLE_FLAGS = Self::PRESENT.bits() | Self::WRITABLE.bits();
    }
}

#[derive(Clone, Copy, Debug)]
pub enum MapError {
    FrameAllocationFailed,
    TranslationFailed,
}

pub struct Mapper<'a, Translator, Allocator> {
    level_4: &'a mut PageTable,
    translator: Translator,
    frame_allocator: Allocator,
    _unsend: core::marker::PhantomData<*const ()>,
}

impl<'a, Translator, Allocator> Mapper<'a, Translator, Allocator>
where
    Translator: FnMut(PhysAddress) -> Option<VirtAddress>,
    Allocator: FnMut() -> Option<Frame>,
{
    /// Create a `Mapper` for the given `level_4` page table, using `translator`
    /// to map physical to virtual addresses. `frame_allocator` is used to get
    /// frames to place new page tables in.
    ///
    /// # Safety
    /// * `level_4` must be a valid L4 page table, and all physical addresses
    ///   referenced from L2+ tables must refer to valid page tables.
    /// * `translator` must return valid accessible virtual addresss for the
    ///   current address space, or `None`.
    /// * `frame_allocator` must return valid physical memory frames not in use
    ///   anywhere else, or `None`.
    /// * If `level_4` is the active page table, client must ensure translations
    ///   actively in use are not broken.
    pub unsafe fn new(
        level_4: &'a mut PageTable,
        translator: Translator,
        frame_allocator: Allocator,
    ) -> Self {
        Mapper {
            level_4,
            translator,
            frame_allocator,
            _unsend: core::marker::PhantomData,
        }
    }

    /// Map `page` to `frame` in the table. The leaf table entry will have
    /// `leaf_flags`. All parent table entries, if already present, will have
    /// their flags masked with `parent_mask_flags`, then those in
    /// `parent_set_flags` will be set. If not present, a new table will be
    /// allocated and the parent entry will have `parent_set_flags`.
    ///
    /// Note that this currently will overwrite any existing leaf entries.
    pub unsafe fn map(
        &mut self,
        page: Page,
        frame: Frame,
        leaf_flags: PageTableFlags,
        parent_set_flags: PageTableFlags,
        parent_mask_flags: PageTableFlags,
    ) -> Result<(), MapError> {
        let l4e: &mut PageTableEntry = &mut self.level_4.entries[page.l4_index()];
        // SAFETY: each traversal requires that the passed entry is a valid
        // entry in a non-leaf table. We know this to be the case for each call.
        let l3: &mut PageTable = unsafe {
            Self::next_level_alloc(
                l4e,
                &mut self.translator,
                &mut self.frame_allocator,
                parent_set_flags,
                parent_mask_flags,
            )?
        };
        let l3e = &mut l3.entries[page.l3_index()];
        let l2: &mut PageTable = unsafe {
            Self::next_level_alloc(
                l3e,
                &mut self.translator,
                &mut self.frame_allocator,
                parent_set_flags,
                parent_mask_flags,
            )?
        };
        let l2e = &mut l2.entries[page.l2_index()];
        let l1: &mut PageTable = unsafe {
            Self::next_level_alloc(
                l2e,
                &mut self.translator,
                &mut self.frame_allocator,
                parent_set_flags,
                parent_mask_flags,
            )?
        };
        let mut l1e = PageTableEntry::zero();
        // TODO: handle existing mapping.
        l1e.set_addr(frame.start());
        l1e.set_flags(leaf_flags);
        unsafe {
            compiler_fence(Ordering::AcqRel);
            ptr::write_volatile(&mut l1.entries[page.l1_index()] as *mut _, l1e);
            compiler_fence(Ordering::AcqRel);
        }

        Ok(())
    }

    /// Traverse from `entry` in a parent table to the lower-level table it
    /// points to. If it is not present, fetches a physical memory frame with
    /// `frame_allocator`, places an empty table there, and points `entry` to it
    /// with `set_flags`. If it is, & masks `entry` flags with `mask_flags`
    /// then sets those in `set_flags` and otherwise does not modify the entry.
    ///
    /// `translator` is used to map physical to virtual addresses to access the
    /// next table. `translator` and `frame_allocator` must abide by the same
    /// contract specified for `new()`. `entry` must be in a parent table, not a
    /// leaf table.
    ///
    /// Returns a mutable reference to the next table or an error.
    #[inline]
    unsafe fn next_level_alloc<'b>(
        entry: &'b mut PageTableEntry,
        translator: &mut Translator,
        frame_allocator: &mut Allocator,
        set_flags: PageTableFlags,
        mask_flags: PageTableFlags,
    ) -> Result<&'b mut PageTable, MapError> {
        let mut translate = |phys: PhysAddress| {
            let virt = translator(phys).ok_or(MapError::TranslationFailed)?;
            assert!(!virt.is_zero());
            assert!(virt.is_aligned_to(4096), "{virt:?}");
            Ok(virt.as_mut_ptr())
        };

        // NOTE: here we assume that if the PRESENT flag is not set, then this
        // entry does not "own" a valid frame. If this were not the case we'd
        // leak a frame. This is not unsafe, but it is a case to watch out for.
        let next_table_ptr: *mut PageTable = if entry.get_flags().contains(PageTableFlags::PRESENT)
        {
            let new_flags = entry.get_flags() & mask_flags | set_flags;
            entry.set_flags(new_flags);
            translate(entry.get_addr())?
        } else {
            // Allocate a new frame to hold the next level table and zero it.
            let new_frame = frame_allocator().ok_or(MapError::FrameAllocationFailed)?;
            let ptr = translate(new_frame.start())?;
            unsafe {
                ptr::write(ptr, PageTable::zero());
            }
            entry.set_addr(new_frame.start());
            entry.set_flags(set_flags.union(PageTableFlags::PRESENT));
            ptr
        };

        // SAFETY: given the assumptions:
        // 1. If applicable, `new_frame` above was a valid unused frame.
        // 2. `entry.get_addr()` references a valid physical frame that is not
        //    referenced by any other page tables.
        // 3. `next_table_addr` is a valid mapping of the frame into the current
        //    virtual address space.
        //
        // ... this is sound. (1) and (3) rely on the client upholding their
        // contract. (2) relies on us upholding our invariants.
        unsafe { Ok(&mut *next_table_ptr) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The address field and the flag field must never overlap; otherwise
    /// `set_addr`/`set_flags` would clobber each other.
    #[test]
    fn flag_bits_and_addr_bits_are_disjoint() {
        assert_eq!(PAGE_TABLE_ENTRY_ADDR_BITS & PageTableFlags::all().bits(), 0);
    }

    /// Regression: `set_flags` must assign, not OR-accumulate. Narrowing the
    /// flag set has to actually clear the flags that are no longer present.
    #[test]
    fn set_flags_replaces_rather_than_accumulates() {
        let mut e = PageTableEntry::zero();
        e.set_flags(PageTableFlags::PRESENT | PageTableFlags::WRITABLE);
        assert_eq!(
            e.get_flags().bits(),
            (PageTableFlags::PRESENT | PageTableFlags::WRITABLE).bits()
        );

        e.set_flags(PageTableFlags::PRESENT);
        assert_eq!(e.get_flags().bits(), PageTableFlags::PRESENT.bits());
    }

    /// Regression: `set_addr` must assign, not OR-accumulate. Setting a second
    /// address has to replace the first, not merge their bits.
    #[test]
    fn set_addr_replaces_rather_than_accumulates() {
        let a = PhysAddress::from_raw(0x1_4000_5000);
        let b = PhysAddress::from_raw(0x2_8000_a000);
        let mut e = PageTableEntry::zero();

        e.set_addr(a);
        assert_eq!(e.get_addr(), a);

        e.set_addr(b);
        assert_eq!(e.get_addr(), b);
    }

    /// Setting the address must never disturb previously-set flags.
    #[test]
    fn set_addr_preserves_flags() {
        let mut e = PageTableEntry::zero();
        e.set_flags(PageTableFlags::PRESENT | PageTableFlags::WRITABLE);
        e.set_addr(PhysAddress::from_raw(0x1_4000_5000));
        assert_eq!(
            e.get_flags().bits(),
            (PageTableFlags::PRESENT | PageTableFlags::WRITABLE).bits()
        );
    }

    /// Setting flags must never disturb the stored address.
    #[test]
    fn set_flags_preserves_addr() {
        let addr = PhysAddress::from_raw(0x1234_5000);
        let mut e = PageTableEntry::zero();
        e.set_addr(addr);
        e.set_flags(PageTableFlags::PRESENT | PageTableFlags::WRITABLE);
        assert_eq!(e.get_addr(), addr);

        e.set_flags(PageTableFlags::PRESENT);
        assert_eq!(e.get_addr(), addr);
        assert_eq!(e.get_flags().bits(), PageTableFlags::PRESENT.bits());
    }

    /// Regression: the address field spans the full 52-bit physical range, so
    /// an address with bits set above bit 47 must round-trip (the old 36-bit
    /// mask silently truncated it).
    #[test]
    fn set_addr_round_trips_above_2_pow_48() {
        let addr = PhysAddress::from_raw(1 << 48);
        let mut e = PageTableEntry::zero();
        e.set_addr(addr);
        assert_eq!(e.get_addr(), addr);
    }

    /// `MAX_PHYS_ADDR` is exclusive: setting it (2^52) must panic.
    #[test]
    #[should_panic]
    fn set_addr_panics_at_max() {
        let mut e = PageTableEntry::zero();
        e.set_addr(MAX_PHYS_ADDR);
    }

    use proptest::prelude::*;

    proptest! {
        /// Address and flags are stored in disjoint bit ranges: any aligned
        /// address below the max and any flag combination must both round-trip,
        /// regardless of the order they were set.
        #[test]
        fn addr_and_flags_round_trip(
            frame_number in 0u64..(1 << 40),
            raw_flags in any::<u64>(),
        ) {
            let addr = PhysAddress::from_raw(frame_number << 12);
            let flags = PageTableFlags::from_bits_truncate(raw_flags);

            let mut e = PageTableEntry::zero();
            e.set_addr(addr);
            e.set_flags(flags);

            prop_assert_eq!(e.get_addr(), addr);
            prop_assert_eq!(e.get_flags().bits(), flags.bits());
        }
    }
}

/// End-to-end tests that drive the real `Mapper` against a fake physical
/// address space, then check the result with an independent `translate` oracle
/// that walks the produced page tables. The unit tests above only exercise a
/// single `PageTableEntry`; these reach the multi-level traversal, parent-table
/// allocation and reuse, and parent-flag masking in `map`/`next_level_alloc` —
/// the logic most likely to harbor an addressing or aliasing bug.
///
/// The harness deliberately routes every table access the way real hardware
/// would: the `Mapper` only ever sees *physical* frame addresses and must go
/// through the `translator` to touch a table. That is also why these tests are
/// the natural target for Miri (`cargo smiri`, see AGENTS.md): the whole point
/// is to run the unsafe pointer walks under a checker.
#[cfg(test)]
mod harness_tests {
    use super::*;
    use crate::memory::addr::{Length, PhysAddress, VirtAddress};
    use crate::memory::page::{Frame, Page, PAGE_SIZE};

    use core::cell::Cell;

    use std::boxed::Box;
    use std::vec::Vec;

    /// A fake physical address space backing the page-table frames handed to the
    /// `Mapper`.
    ///
    /// The storage is pre-allocated to a fixed size and never reallocated, so a
    /// raw pointer the translator hands out during one `map` call stays valid
    /// for the rest of the test. (A growing `Vec` would move its buffer and
    /// leave the `Mapper` writing through a dangling pointer — exactly the kind
    /// of bug Miri would flag, but not one we want to inject ourselves.)
    struct FakePhysMem {
        /// Backing store for allocatable page-table frames; one 4 KiB
        /// `PageTable` per frame. Boxed so the address is stable.
        storage: Box<[PageTable]>,
        /// Fake physical address of `storage[0]`. Chosen far from any plausible
        /// host pointer value so that confusing a physical address for a
        /// host/virtual one would be caught rather than silently "working".
        base: PhysAddress,
        /// Index of the next unallocated frame in `storage`. `Cell` so the
        /// allocator and translator can both borrow `&self` (the allocator only
        /// needs to bump this counter, not `&mut` the whole arena).
        next: Cell<usize>,
    }

    impl FakePhysMem {
        fn new(num_frames: usize) -> FakePhysMem {
            let storage = (0..num_frames)
                .map(|_| PageTable::zero())
                .collect::<Vec<_>>()
                .into_boxed_slice();
            FakePhysMem {
                storage,
                // 1 GiB: arbitrary, frame-aligned, and well clear of host
                // pointers.
                base: PhysAddress::from_raw(1 << 30),
                next: Cell::new(0),
            }
        }

        /// Fake physical address of frame `ndx` in the arena.
        fn frame_phys(&self, ndx: usize) -> PhysAddress {
            self.base + Length::from_raw(ndx as u64 * PAGE_SIZE.as_raw())
        }

        /// Host pointer to the table stored at fake-physical address `phys`.
        ///
        /// Panics if `phys` is not one of the frames we handed out — the
        /// `Mapper` must only ever translate page-table frames, never a leaf
        /// target frame, so an out-of-arena translation is a real bug.
        fn phys_to_host(&self, phys: PhysAddress) -> *mut PageTable {
            let off = phys
                .as_raw()
                .checked_sub(self.base.as_raw())
                .expect("translated a physical address below the arena");
            assert_eq!(off % PAGE_SIZE.as_raw(), 0, "translated an unaligned frame");
            let ndx = (off / PAGE_SIZE.as_raw()) as usize;
            assert!(ndx < self.storage.len(), "translated past the arena");
            // Derive from the arena's own pointer so the provenance is real.
            // `map` will funnel this address through `VirtAddress` (a bare
            // `u64`) and back out via `as_mut_ptr`, i.e. an int->ptr round trip;
            // under Miri's *permissive* provenance that resolves back to this
            // allocation because `VirtAddress::from_ptr` exposes it below.
            let base_ptr = self.storage.as_ptr() as *mut PageTable;
            // SAFETY: `ndx` is in bounds per the assert above.
            unsafe { base_ptr.add(ndx) }
        }

        /// Translator for `Mapper::new`: fake-physical -> host/virtual.
        fn translate(&self, phys: PhysAddress) -> Option<VirtAddress> {
            Some(VirtAddress::from_ptr(self.phys_to_host(phys)))
        }

        /// Frame allocator for `Mapper::new`: hand out the next arena frame.
        fn alloc(&self) -> Option<Frame> {
            let ndx = self.next.get();
            if ndx >= self.storage.len() {
                return None;
            }
            self.next.set(ndx + 1);
            Some(Frame::new(self.frame_phys(ndx)))
        }

        /// Number of frames allocated so far.
        fn allocated(&self) -> usize {
            self.next.get()
        }

        /// The `translate` **oracle**: independently walk the page tables from
        /// `root` and resolve `page`, returning its leaf entry. Returns `None`
        /// if any level along the way is not present. This deliberately does
        /// not share code with `map`; it is the check, not the thing checked.
        fn walk(&self, root: &PageTable, page: Page) -> Option<PageTableEntry> {
            let l3 = self.descend(root.entries()[page.l4_index()])?;
            let l2 = self.descend(l3.entries()[page.l3_index()])?;
            let l1 = self.descend(l2.entries()[page.l2_index()])?;
            let leaf = l1.entries()[page.l1_index()];
            leaf.get_flags()
                .contains(PageTableFlags::PRESENT)
                .then_some(leaf)
        }

        /// Follow a parent-table entry to the table it points at, or `None` if
        /// it isn't present.
        fn descend(&self, mut entry: PageTableEntry) -> Option<&PageTable> {
            if !entry.get_flags().contains(PageTableFlags::PRESENT) {
                return None;
            }
            let ptr = self.phys_to_host(entry.get_addr());
            // SAFETY: `ptr` points at a live, initialized `PageTable` in the
            // arena (every frame we hand out is zeroed `PageTable` storage, and
            // the `Mapper` only writes valid tables there). Mapping is complete
            // before any oracle walk runs, so there is no concurrent writer.
            Some(unsafe { &*ptr })
        }
    }

    // Reading the descend/walk raw-pointer chases under Miri is the point of the
    // harness; see AGENTS.md ("Verifying changes") for `cargo smiri`.

    /// Build a canonical, page-aligned lower-half virtual address from its four
    /// 9-bit table indices.
    fn virt(l4: usize, l3: usize, l2: usize, l1: usize) -> VirtAddress {
        let raw = ((l4 as u64) << 39)
            | ((l3 as u64) << 30)
            | ((l2 as u64) << 21)
            | ((l1 as u64) << 12);
        VirtAddress::from_raw(raw)
    }

    fn page(l4: usize, l3: usize, l2: usize, l1: usize) -> Page {
        Page::new(virt(l4, l3, l2, l1))
    }

    #[test]
    fn single_map_round_trips() {
        let mem = FakePhysMem::new(64);
        let mut root = PageTable::zero();

        let p = page(1, 2, 3, 4);
        let f = Frame::new(PhysAddress::from_raw(0x8000_0000));
        let leaf_flags =
            PageTableFlags::PRESENT | PageTableFlags::WRITABLE | PageTableFlags::EXECUTE_DISABLE;
        let parent_flags = PageTableFlags::PRESENT | PageTableFlags::WRITABLE;

        {
            let mut mapper =
                unsafe { Mapper::new(&mut root, |p| mem.translate(p), || mem.alloc()) };
            unsafe {
                mapper
                    .map(p, f, leaf_flags, parent_flags, PageTableFlags::all())
                    .unwrap();
            }
        }

        // Three parent tables (L3, L2, L1) were allocated.
        assert_eq!(mem.allocated(), 3);

        let leaf = mem.walk(&root, p).expect("page should be mapped");
        assert_eq!(leaf.get_addr(), f.start());
        assert_eq!(leaf.get_flags().bits(), leaf_flags.bits());

        // The L4 parent entry got the parent flags plus PRESENT.
        let l4e = root.entries()[p.l4_index()];
        assert!(l4e.get_flags().contains(PageTableFlags::PRESENT));
        assert!(l4e.get_flags().contains(PageTableFlags::WRITABLE));
    }

    #[test]
    fn unmapped_page_translates_to_none() {
        let mem = FakePhysMem::new(64);
        let mut root = PageTable::zero();

        let mapped = page(1, 2, 3, 4);
        let f = Frame::new(PhysAddress::from_raw(0x8000_0000));
        {
            let mut mapper =
                unsafe { Mapper::new(&mut root, |p| mem.translate(p), || mem.alloc()) };
            unsafe {
                mapper
                    .map(
                        mapped,
                        f,
                        PageTableFlags::PRESENT,
                        PageTableFlags::PRESENT,
                        PageTableFlags::all(),
                    )
                    .unwrap();
            }
        }

        // A different page in a different L4 slot is not mapped.
        assert!(mem.walk(&root, page(9, 2, 3, 4)).is_none());
        // A different leaf under the *same* parents is also not mapped.
        assert!(mem.walk(&root, page(1, 2, 3, 5)).is_none());
    }

    #[test]
    fn shared_parent_tables_are_reused() {
        let mem = FakePhysMem::new(64);
        let mut root = PageTable::zero();

        // Two pages differing only in their L1 index share all three parent
        // tables, so only 3 frames total should be allocated.
        let a = page(1, 2, 3, 4);
        let b = page(1, 2, 3, 5);
        let fa = Frame::new(PhysAddress::from_raw(0x8000_0000));
        let fb = Frame::new(PhysAddress::from_raw(0x8000_1000));
        let flags = PageTableFlags::PRESENT | PageTableFlags::WRITABLE;

        {
            let mut mapper =
                unsafe { Mapper::new(&mut root, |p| mem.translate(p), || mem.alloc()) };
            unsafe {
                mapper.map(a, fa, flags, flags, PageTableFlags::all()).unwrap();
                mapper.map(b, fb, flags, flags, PageTableFlags::all()).unwrap();
            }
        }

        assert_eq!(mem.allocated(), 3, "parent tables should be reused");
        assert_eq!(mem.walk(&root, a).unwrap().get_addr(), fa.start());
        assert_eq!(mem.walk(&root, b).unwrap().get_addr(), fb.start());
    }

    #[test]
    fn distinct_l4_slots_allocate_separate_subtrees() {
        let mem = FakePhysMem::new(64);
        let mut root = PageTable::zero();

        let a = page(1, 0, 0, 0);
        let b = page(2, 0, 0, 0);
        let fa = Frame::new(PhysAddress::from_raw(0x8000_0000));
        let fb = Frame::new(PhysAddress::from_raw(0x8000_1000));
        let flags = PageTableFlags::PRESENT | PageTableFlags::WRITABLE;

        {
            let mut mapper =
                unsafe { Mapper::new(&mut root, |p| mem.translate(p), || mem.alloc()) };
            unsafe {
                mapper.map(a, fa, flags, flags, PageTableFlags::all()).unwrap();
                mapper.map(b, fb, flags, flags, PageTableFlags::all()).unwrap();
            }
        }

        // Two disjoint subtrees: 3 tables each.
        assert_eq!(mem.allocated(), 6);
        assert_eq!(mem.walk(&root, a).unwrap().get_addr(), fa.start());
        assert_eq!(mem.walk(&root, b).unwrap().get_addr(), fb.start());
    }

    #[test]
    fn parent_flags_are_masked_then_set_on_reuse() {
        let mem = FakePhysMem::new(64);
        let mut root = PageTable::zero();

        // First map establishes WRITABLE parents.
        let a = page(1, 2, 3, 4);
        // Second map reuses the same parents but masks WRITABLE out and does not
        // re-set it, so the parent entries must lose WRITABLE.
        let b = page(1, 2, 3, 5);
        let f = Frame::new(PhysAddress::from_raw(0x8000_0000));
        let leaf = PageTableFlags::PRESENT;

        let set_first = PageTableFlags::PRESENT | PageTableFlags::WRITABLE;
        let mask_clear_writable = PageTableFlags::all().difference(PageTableFlags::WRITABLE);

        {
            let mut mapper =
                unsafe { Mapper::new(&mut root, |p| mem.translate(p), || mem.alloc()) };
            unsafe {
                mapper
                    .map(a, f, leaf, set_first, PageTableFlags::all())
                    .unwrap();
                mapper
                    .map(b, f, leaf, PageTableFlags::PRESENT, mask_clear_writable)
                    .unwrap();
            }
        }

        // The shared L4 parent should have had WRITABLE masked away.
        let l4e = root.entries()[a.l4_index()];
        assert!(l4e.get_flags().contains(PageTableFlags::PRESENT));
        assert!(
            !l4e.get_flags().contains(PageTableFlags::WRITABLE),
            "WRITABLE should have been masked out on reuse"
        );
    }

    #[test]
    fn high_physical_frame_round_trips_through_full_map() {
        let mem = FakePhysMem::new(64);
        let mut root = PageTable::zero();

        // A leaf frame with a bit set above bit 47 — the exact range the old
        // 36-bit address mask silently truncated. It must survive a full
        // map + oracle walk.
        let p = page(5, 6, 7, 8);
        let f = Frame::new(PhysAddress::from_raw(1 << 48));
        let flags = PageTableFlags::PRESENT | PageTableFlags::WRITABLE;

        {
            let mut mapper =
                unsafe { Mapper::new(&mut root, |p| mem.translate(p), || mem.alloc()) };
            unsafe {
                mapper.map(p, f, flags, flags, PageTableFlags::all()).unwrap();
            }
        }

        assert_eq!(mem.walk(&root, p).unwrap().get_addr(), f.start());
    }

    #[test]
    fn remap_replaces_leaf() {
        let mem = FakePhysMem::new(64);
        let mut root = PageTable::zero();

        let p = page(1, 2, 3, 4);
        let fa = Frame::new(PhysAddress::from_raw(0x8000_0000));
        let fb = Frame::new(PhysAddress::from_raw(0x9000_0000));
        let flags_a = PageTableFlags::PRESENT | PageTableFlags::WRITABLE;
        let flags_b = PageTableFlags::PRESENT | PageTableFlags::EXECUTE_DISABLE;

        {
            let mut mapper =
                unsafe { Mapper::new(&mut root, |p| mem.translate(p), || mem.alloc()) };
            unsafe {
                mapper.map(p, fa, flags_a, flags_a, PageTableFlags::all()).unwrap();
                mapper.map(p, fb, flags_b, flags_b, PageTableFlags::all()).unwrap();
            }
        }

        // Same page, remapped: only the original 3 tables, leaf now points at
        // the new frame with the new flags (map builds a fresh leaf entry).
        assert_eq!(mem.allocated(), 3);
        let leaf = mem.walk(&root, p).unwrap();
        assert_eq!(leaf.get_addr(), fb.start());
        assert_eq!(leaf.get_flags().bits(), flags_b.bits());
    }
}
