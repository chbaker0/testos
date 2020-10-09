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

    pub fn distance_from(&self, right: &Address) -> Length {
        assert!(self >= right);
        Length::from_raw(self.as_raw() - right.as_raw())
    }

    pub fn distance_to(&self, right: &Address) -> Length {
        assert!(self <= right);
        Length::from_raw(right.as_raw() - self.as_raw())
    }

    pub fn offset_by(&self, length: &Length) -> Address {
        assert!(length.as_raw() <= u64::MAX - self.as_raw());
        Self::from_raw(self.as_raw() + length.as_raw())
    }

    /// Returns the first address below `self` that is aligned to `alignment`,
    /// which must be a power of two.
    pub fn align_down(&self, alignment: usize) -> Address {
        Self::from_raw(align_u64_down(self.as_raw(), alignment))
    }

    /// Returns the first address above `self` that is aligned to `alignment`,
    /// which must be a power of two.
    pub fn align_up(&self, alignment: usize) -> Address {
        Self::from_raw(align_u64_up(self.as_raw(), alignment))
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
    pub fn align_down(&self, alignment: usize) -> Length {
        Length::from_raw(align_u64_down(self.as_raw(), alignment))
    }

    /// Returns the first length greater than `self` that is aligned to `alignment`,
    /// which must be a power of two.
    pub fn align_up(&self, alignment: usize) -> Length {
        Length::from_raw(align_u64_up(self.as_raw(), alignment))
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

    pub fn address(&self) -> Address {
        self.address
    }

    pub fn length(&self) -> Length {
        self.length
    }

    pub fn end_address(&self) -> Address {
        self.address.offset_by(&self.length)
    }

    /// Returns the largest extent completely contained in `self` whose start
    /// and end addresses are aligned to `alignment`. `alignment` must be a
    /// power of two.
    pub fn shrink_to_alignment(&self, alignment: usize) -> Option<Extent> {
        let start_address = self.address.align_up(alignment);
        let end_address = self.end_address().align_down(alignment);
        if end_address <= start_address {
            None
        } else {
            Some(Extent {
                address: start_address,
                length: start_address.distance_to(&end_address),
            })
        }
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
        panic!("...");
    }
}

/// Given power-of-two `alignment`, returns the largest value below `x` aligned
/// to `alignment`
const fn align_u64_down(x: u64, alignment: usize) -> u64 {
    let mask = !(alignment as u64 - 1);
    x & mask
}

/// Given power-of-two `alignment`, returns the smallest value above `x` aligned
/// to `alignment`
const fn align_u64_up(x: u64, alignment: usize) -> u64 {
    align_u64_down(x + (alignment - 1) as u64, alignment)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn align_raw() {
        assert_eq!(align_u64_down(0, 2), 0);
        assert_eq!(align_u64_down(1, 2), 0);
        assert_eq!(align_u64_down(2, 2), 2);

        assert_eq!(align_u64_up(0, 2), 0);
        assert_eq!(align_u64_up(1, 2), 2);
        assert_eq!(align_u64_up(2, 2), 2);

        assert_eq!(align_u64_down(255, 1024), 0);
        assert_eq!(align_u64_up(255, 1024), 1024);
    }

    #[test]
    fn align_address() {
        assert_eq!(Address::from_raw(0).align_down(1024), Address::from_raw(0));
        assert_eq!(Address::from_raw(0).align_up(1024), Address::from_raw(0));

        assert_eq!(
            Address::from_raw(1024).align_down(1024),
            Address::from_raw(1024)
        );
        assert_eq!(
            Address::from_raw(1024).align_up(1024),
            Address::from_raw(1024)
        );

        assert_eq!(Address::from_raw(1).align_down(1024), Address::from_raw(0));
        assert_eq!(Address::from_raw(1).align_up(1024), Address::from_raw(1024));

        assert_eq!(
            Address::from_raw(1023).align_down(1024),
            Address::from_raw(0)
        );
        assert_eq!(
            Address::from_raw(1023).align_up(1024),
            Address::from_raw(1024)
        );
    }

    #[test]
    fn shrink_extent() {
        let extent = Extent::new(Address::from_raw(1), Length::from_raw(8191))
            .shrink_to_alignment(4096)
            .unwrap();
        assert_eq!(
            extent,
            Extent::new(Address::from_raw(4096), Length::from_raw(4096))
        );

        let extent = Extent::new(Address::from_raw(0), Length::from_raw(4097))
            .shrink_to_alignment(4096)
            .unwrap();
        assert_eq!(
            extent,
            Extent::new(Address::from_raw(0), Length::from_raw(4096))
        );

        let extent = Extent::new(Address::from_raw(4095), Length::from_raw(4097))
            .shrink_to_alignment(4096)
            .unwrap();
        assert_eq!(
            extent,
            Extent::new(Address::from_raw(4096), Length::from_raw(4096))
        );
    }

    #[test]
    fn shrink_extent_already_aligned() {
        // An already-aligned extent should not be shrunk.
        let extent = Extent::new(Address::from_raw(0), Length::from_raw(4096));
        assert_eq!(extent, extent.shrink_to_alignment(4096).unwrap());

        let extent = Extent::new(Address::from_raw(4096), Length::from_raw(8192));
        assert_eq!(extent, extent.shrink_to_alignment(4096).unwrap());
    }

    #[test]
    fn shrink_extent_empty() {
        // If there's no aligned sub-extent, it must return None.
        let extent =
            Extent::new(Address::from_raw(1), Length::from_raw(4096)).shrink_to_alignment(4096);
        assert_eq!(extent, None);

        let extent =
            Extent::new(Address::from_raw(0), Length::from_raw(4095)).shrink_to_alignment(4096);
        assert_eq!(extent, None);

        let extent =
            Extent::new(Address::from_raw(1), Length::from_raw(8190)).shrink_to_alignment(4096);
        assert_eq!(extent, None);
    }
}
