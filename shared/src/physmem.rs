use core::iter::IntoIterator;

use arrayvec::ArrayVec;

#[derive(Clone, Copy, Eq, Ord, PartialEq, PartialOrd, Debug, Hash)]
pub struct Address(u64);

impl Address {
    pub fn from_raw(val: u64) -> Address {
        Address(val)
    }

    pub fn as_raw(&self) -> u64 {
        self.0
    }

    /// Returns the first address below `self` that is aligned to `alignment`,
    /// which must be a power of two.
    pub fn align_down(&self, alignment: usize) -> Address {
        Address(align_u64_down(self.0, alignment))
    }

    /// Returns the first address above `self` that is aligned to `alignment`,
    /// which must be a power of two.
    pub fn align_up(&self, alignment: usize) -> Address {
        Address(align_u64_up(self.0, alignment))
    }
}

#[derive(Clone, Copy, Eq, Ord, PartialEq, PartialOrd, Debug, Hash)]
pub struct Length(u64);

impl Length {
    pub fn from_raw(val: u64) -> Length {
        Length(val)
    }

    pub fn as_raw(&self) -> u64 {
        self.0
    }

    /// Returns the first length lesser than `self` that is aligned to `alignment`,
    /// which must be a power of two.
    pub fn align_down(&self, alignment: usize) -> Address {
        Address(align_u64_down(self.0, alignment))
    }

    /// Returns the first length greater than `self` that is aligned to `alignment`,
    /// which must be a power of two.
    pub fn align_up(&self, alignment: usize) -> Address {
        Address(align_u64_up(self.0, alignment))
    }
}

#[derive(Clone, Copy, Eq, PartialEq, Debug, Hash)]
pub struct Extent {
    pub address: Address,
    pub length: Length,
}

impl Extent {
    pub fn new(address: Address, length: Length) -> Extent {
        assert_ne!(0, length.as_raw());
        Extent {
            address: address,
            length: length,
        }
    }

    /// Returns the largest extent completely contained in `self` whose start
    /// and end addresses are aligned to `alignment`. `alignment` must be a
    /// power of two.
    pub fn shrink_to_alignment(&self, alignment: usize) -> Option<Extent> {
        let new_address = self.address.align_up(alignment);
    }
}

/// A map of the machine's physical memory.
pub struct Map {
    entries: ArrayVec<[MapEntry; 128]>,
}

impl Map {
    pub fn from_entries<T: IntoIterator<Item = MapEntry>>(src: T) -> Map {
        Map {
            entries: src.into_iter().collect(),
        }
    }

    pub fn entries(&self) -> &[MapEntry] {
        &self.entries
    }
}

#[derive(Clone, Copy, Debug)]
pub struct MapEntry {
    pub extent: Extent,
    pub mem_type: MemoryType,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MemoryType {
    /// Available for use
    Available,
    /// Contains ACPI information, but otherwise usable
    Acpi,
    /// Reserved and must be preserved on hibernation
    ReservedPreserveOnHibernation,
    /// Corrupt and unusable
    Defective,
    /// Cannot be used
    Reserved,
}

/// Allocates physical pages of a given size from the lowest to highest address.
/// Does not support freeing.
pub struct BumpAllocator {
    available: ArrayVec<[Extent; 128]>,
    page_size: usize,
}

impl BumpAllocator {
    /// Allocates from the available memory in `map` after removing specified
    /// regions in `reserved`. Allocations are of size `page_size`, which must
    /// be a power of two.
    ///
    /// Addresses in `map` and `reserved` do not need to be aligned. However,
    /// all returned allocations will be aligned to `page_size`.
    pub fn new<T: IntoIterator<Item = Extent>>(
        page_size: usize,
        map: &Map,
        reserved: T,
    ) -> BumpAllocator {
    }
}

const fn align_u64_down(x: u64, alignment: usize) -> u64 {
    assert!(alignment.is_power_of_two());
    let mask = !(alignment as u64 - 1);
    x & mask
}

const fn align_u64_up(x: u64, alignment: usize) -> u64 {
    align_u64_down(x + (alignment - 1) as u64, alignment)
}
