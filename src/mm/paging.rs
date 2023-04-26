use shared::memory::{addr::*, page::*};

use core::ptr;
use core::sync::atomic::{compiler_fence, Ordering};

use static_assertions as sa;

pub const MAX_PHYS_ADDR_BITS: u32 = 52;
pub const MAX_PHYS_ADDR: PhysAddress = PhysAddress::from_raw(2 << MAX_PHYS_ADDR_BITS);

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
}

// Assert that `PageTable` is 4 KiB.
sa::assert_eq_size!(PageTable, [u8; 4096]);

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
        // Page table entries are essentially an aligned physical addresses with
        // flag bits OR'ed in. Bits 0-11 and 52-63 of the address always zero
        // due to the alignment requirement and the maximum address. These are
        // used as paging flags.
        self.raw |= addr.as_raw();
    }

    #[inline]
    pub fn get_addr(&self) -> PhysAddress {
        PhysAddress::from_raw(self.raw & PAGE_TABLE_ENTRY_ADDR_BITS)
    }

    /// Set flags (as documented in `PageTableFlags`).
    #[inline]
    pub fn set_flags(&mut self, flags: PageTableFlags) {
        self.raw |= flags.bits();
    }

    /// Get flags (as documented in `PageTableFlags`).
    #[inline]
    pub fn get_flags(&mut self) -> PageTableFlags {
        // SAFETY: PageTableFlags::all().bits() only returns bits valid for
        // PageTableFlags. Bitwise-and with any other value will yield only
        // valid bits.
        unsafe { PageTableFlags::from_bits_unchecked(self.raw & PageTableFlags::all().bits()) }
    }
}

pub const PAGE_TABLE_ENTRY_ADDR_BITS: u64 = ((1 << 36) - 1) << 12;

bitflags::bitflags! {
    /// Control bits for a page table entry. Documented in architecture manual.
    /// Note that some bits may not be valid for some table levels, and not
    /// every combination of bits may be valid.
    ///
    /// Entries prefixed with `APP_` are from "available" bits, so any meaning
    /// is attributed by us.
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

        const DEFAULT_PARENT_TABLE_FLAGS = Self::PRESENT.bits | Self::WRITABLE.bits;
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
