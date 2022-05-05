use shared::memory::{page::PAGE_SIZE, PhysAddress};

use static_assertions as sa;

pub const MAX_PHYS_ADDR_BITS: u32 = 52;
pub const MAX_PHYS_ADDR: PhysAddress = PhysAddress::from_raw(2 << MAX_PHYS_ADDR_BITS);

#[derive(Clone, Debug)]
#[repr(C, align(4096))]
pub struct PageTable {
    entries: [PageTableEntry; 512],
}

// Assert that `PageTable` is 4 KiB.
sa::assert_eq_size!(PageTable, [u8; 4096]);

#[derive(Clone, Debug)]
#[repr(transparent)]
pub struct PageTableEntry {
    raw: u64,
}

impl PageTableEntry {
    /// Create an entry with all bits set to zero.
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
    pub fn set_addr(&mut self, addr: PhysAddress) {
        assert!(addr.is_aligned_to_length(PAGE_SIZE), "{addr:?}");
        assert!(addr < MAX_PHYS_ADDR);
        // Page table entries are essentially an aligned physical addresses with
        // flag bits OR'ed in. Bits 0-11 and 52-63 of the address always zero
        // due to the alignment requirement and the maximum address. These are
        // used as paging flags.
        self.raw |= addr.as_raw();
    }

    pub fn get_addr(&self) -> PhysAddress {
        PhysAddress::from_raw(self.raw & PAGE_TABLE_ENTRY_ADDR_BITS)
    }

    /// Set flags (as documented in `PageTableFlags`).
    pub fn set_flags(&mut self, flags: PageTableFlags) {
        self.raw |= flags.bits();
    }
}

pub const PAGE_TABLE_ENTRY_ADDR_BITS: u64 = ((1 << 36) - 1) << 12;

bitflags::bitflags! {
    /// Control bits for a page table entry. Documented in architecture manual.
    /// Note that some bits may not be valid for some table levels, and not
    /// every combination of bits may be valid.
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
    }
}

pub struct Mapper<'a, Translator> {
    level_4: &'a mut PageTable,
    translator: Translator,
}

impl<'a, Translator> !Send for Mapper<'a, Translator> {}
impl<'a, Translator> !Sync for Mapper<'a, Translator> {}

impl<'a, Translator: FnMut(PhysAddress) -> Option<VirtAddress>> Mapper<'a, Translator> {
    /// Create a `Mapper` for the given `level_4` page table, using `translator`
    /// to map physical to virtual addresses.
    ///
    /// # Safety
    /// * `level_4` must be a valid L4 page table, and all physical addresses
    ///   referenced from L2+ tables must refer to valid page tables.
    /// * `translator` must return valid accessible virtual addresss for the
    ///   current address space, or `None`.
    pub unsafe fn new(level_4: &'a mut PageTable, translator: Translator) -> Self {
        Mapper {
            level_4,
            translator,
        }
    }

    pub unsafe fn map<GetNewFrame: FnMut() -> Option<Frame>>(
        &mut self,
        page: Page,
        frame: Frame,
        flags: PageTableFlags,
    ) {
    }

    pub fn get_l4_entry(&mut self, page: Page) -> &mut PageTableEntry {
        &mut self.level_4.entries[page.l4_index()]
    }

    pub fn get_l3_entry(&mut self, page: Page) -> Option<&mut PageTableEntry> {
        let l4 = self.get_l4_entry(page);
        let l3: *mut PageTable = self.translator(l4.get_addr())?.as_mut_ptr();
        // SAFETY: assuming the invariants required by the other unsafe methods
        // are upheld, we can dereference.
        let l3: &mut PageTable = unsafe { &mut *l3 };
        Some(&mut l3.entries[page.l3_index()])
    }

    pub fn get_l2_entry(&mut self, page: Page) -> Option<&mut PageTableEntry> {
        let l3 = self.get_l3_entry(page);
        let l2: *mut PageTable = self.translator(l3.get_addr())?.as_mut_ptr();
        // SAFETY: assuming the invariants required by the other unsafe methods
        // are upheld, we can dereference.
        let l2: &mut PageTable = unsafe { &mut *l3 };
        Some(&mut l2.entries[page.l2_index()])
    }

    pub fn get_l1_entry(&mut self, page: Page) -> Option<&mut PageTableEntry> {
        let l2 = self.get_l2_entry(page);
        let l1: *mut PageTable = self.translator(l2.get_addr())?.as_mut_ptr();
        // SAFETY: assuming the invariants required by the other unsafe methods
        // are upheld, we can dereference.
        let l1: &mut PageTable = unsafe { &mut *l1 };
        Some(&mut l1.entries[page.l1_index()])
    }
}
