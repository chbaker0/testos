use core::cmp::{max, min};
use core::iter::IntoIterator;

use arrayvec::ArrayVec;
use itertools::put_back;

#[derive(Clone, Copy, Eq, Ord, PartialEq, PartialOrd, Debug, Hash)]
pub struct Address(u64);

impl Address {
    pub fn from_raw(val: u64) -> Address {
        Address(val)
    }

    pub fn as_raw(&self) -> u64 {
        self.0
    }

    pub fn distance_from(&self, left: &Address) -> Length {
        assert!(self >= left);
        Length::from_raw(self.as_raw() - left.as_raw())
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

    pub fn add(&self, rhs: &Length) -> Length {
        Length::from_raw(self.as_raw() + rhs.as_raw())
    }

    pub fn subtract(&self, rhs: &Length) -> Length {
        assert!(self.as_raw() >= rhs.as_raw());
        Length::from_raw(self.as_raw() - rhs.as_raw())
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
        assert!(length.as_raw() <= u64::MAX - address.as_raw());
        Extent {
            address: address,
            length: length,
        }
    }

    pub fn from_raw(address: u64, length: u64) -> Extent {
        Self::new(Address::from_raw(address), Length::from_raw(length))
    }

    pub fn address(&self) -> Address {
        self.address
    }

    pub fn length(&self) -> Length {
        self.length
    }

    /// The first address just outside us, to the right
    pub fn end_address(&self) -> Address {
        self.address.offset_by(&self.length)
    }

    /// The last address in the extent. E.g.
    ///
    ///
    /// ```
    /// use shared::physmem::*;
    /// assert_eq!(Extent::from_raw(0, 4).last_address(), Address::from_raw(3));
    /// ```
    pub fn last_address(&self) -> Address {
        self.address
            .offset_by(&self.length.subtract(&Length::from_raw(1)))
    }

    pub fn overlap(&self, other: &Extent) -> Option<Extent> {
        if self.address > other.address {
            return other.overlap(self);
        }

        let overlap_start = other.address;

        if self.address.distance_to(&overlap_start) >= self.length {
            return None;
        }

        let overlap_length = min(
            self.length
                .subtract(&self.address.distance_to(&overlap_start)),
            other.length,
        );

        Some(Extent {
            address: overlap_start,
            length: overlap_length,
        })
    }

    pub fn has_overlap(&self, other: &Extent) -> bool {
        self.overlap(other).is_some()
    }

    pub fn left_difference(&self, other: &Extent) -> Option<Extent> {
        if self.address >= other.address {
            return None;
        }

        // Since our address is strictly less than `other`'s, we can safely
        // assume the result is non-empty.
        let diff_length = min(self.length, self.address.distance_to(&other.address));

        Some(Extent {
            address: self.address,
            length: diff_length,
        })
    }

    pub fn right_difference(&self, other: &Extent) -> Option<Extent> {
        if self.last_address() <= other.last_address() {
            return None;
        }

        // Since our right endpoint is completely to the left `other`, the right
        // difference is non-empty. Additionally, since `self.end_address() <=
        // u64::MAX + 1`, we can be assured that `other.end_address() <=
        // u64::MAX`.

        let diff_address = max(self.address, other.end_address());
        let diff_length = self
            .length
            .subtract(&diff_address.distance_from(&self.address));

        Some(Extent {
            address: diff_address,
            length: diff_length,
        })
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
    /// `src` must be sorted by start address, and the extents must not overlap.
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
    free: ArrayVec<[Extent; 128]>,
    page_size: usize,
}

impl BumpAllocator {
    /// Allocates from the available memory in `map` after removing specified
    /// regions in `reserved`. Allocations are multiples of `page_size`, which
    /// must be a power of two.
    ///
    /// `reserved` must be sorted.
    ///
    /// Addresses in `map` and `reserved` do not need to be aligned. However,
    /// all returned allocations will be aligned to `page_size`.
    pub fn new<T: IntoIterator<Item = Extent>>(
        page_size: usize,
        map: &Map,
        reserved: T,
    ) -> BumpAllocator {
        let mut map_iter = put_back(
            map.entries()
                .iter()
                .filter(|e| e.mem_type == MemoryType::Available)
                .map(|e| e.extent),
        );
        let mut reserved_iter = put_back(reserved);

        let mut free = ArrayVec::<[Extent; 128]>::new();

        // Insert free blocks into `free`, removing entries in `reserved`.
        while let Some(ext) = map_iter.next() {
            // Remove holes completely before `ext`; they can be ignored.
            while let Some(hole) = reserved_iter.next() {
                if hole.last_address() >= ext.address() {
                    reserved_iter.put_back(hole);
                    break;
                }
            }

            // Get the next hole.
            let hole = match reserved_iter.next() {
                Some(ext) => ext,
                None => break,
            };

            // If `hole` is completely after `ext`, we can put `ext` in `free`.
            // However, we must retain `hole` in case it intersects with a
            // future `ext`.
            if ext.last_address() < hole.address() {
                free.push(ext);
                reserved_iter.put_back(hole);
                continue;
            }

            // We now know `hole` starts in or before `ext`. Get both sides of
            // the difference of `hole` from `ext`.
            let maybe_left = ext.left_difference(&hole);
            let maybe_right = ext.right_difference(&hole);

            if let Some(left) = maybe_left {}
        }

        // `reserved_iter` is done. Copy the remaining entries directly to `free`.
        while let Some(ext) = map_iter.next() {
            free.push(ext);
        }

        BumpAllocator {
            free: free,
            page_size: page_size,
        }
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
    fn overlap_extent() {
        assert_eq!(
            Extent::from_raw(0, 8).overlap(&Extent::from_raw(0, 8)),
            Some(Extent::from_raw(0, 8))
        );

        assert_eq!(
            Extent::from_raw(0, 8).overlap(&Extent::from_raw(8, 8)),
            None
        );
        assert_eq!(
            Extent::from_raw(0, 8).overlap(&Extent::from_raw(1024, 8)),
            None
        );

        assert_eq!(
            Extent::from_raw(5, 5).overlap(&Extent::from_raw(8, 7)),
            Some(Extent::from_raw(8, 2))
        );
        assert_eq!(
            Extent::from_raw(8, 7).overlap(&Extent::from_raw(5, 5)),
            Some(Extent::from_raw(8, 2))
        );

        assert_eq!(
            Extent::from_raw(0, 10).overlap(&Extent::from_raw(2, 3)),
            Some(Extent::from_raw(2, 3))
        );
        assert_eq!(
            Extent::from_raw(2, 3).overlap(&Extent::from_raw(0, 10)),
            Some(Extent::from_raw(2, 3))
        );
    }

    #[test]
    fn shrink_extent() {
        let extent = Extent::from_raw(1, 8191).shrink_to_alignment(4096).unwrap();
        assert_eq!(extent, Extent::from_raw(4096, 4096));

        let extent = Extent::from_raw(0, 4097).shrink_to_alignment(4096).unwrap();
        assert_eq!(extent, Extent::from_raw(0, 4096));

        let extent = Extent::from_raw(4095, 4097)
            .shrink_to_alignment(4096)
            .unwrap();
        assert_eq!(extent, Extent::from_raw(4096, 4096));
    }

    #[test]
    fn shrink_extent_already_aligned() {
        // An already-aligned extent should not be shrunk.
        let extent = Extent::from_raw(0, 4096);
        assert_eq!(extent, extent.shrink_to_alignment(4096).unwrap());

        let extent = Extent::from_raw(4096, 8192);
        assert_eq!(extent, extent.shrink_to_alignment(4096).unwrap());
    }

    #[test]
    fn shrink_extent_empty() {
        // If there's no aligned sub-extent, it must return None.
        let extent = Extent::from_raw(1, 4096).shrink_to_alignment(4096);
        assert_eq!(extent, None);

        let extent = Extent::from_raw(0, 4095).shrink_to_alignment(4096);
        assert_eq!(extent, None);

        let extent = Extent::from_raw(1, 8190).shrink_to_alignment(4096);
        assert_eq!(extent, None);
    }

    #[test]
    fn left_difference() {
        assert_eq!(
            Extent::from_raw(10, 10).left_difference(&Extent::from_raw(0, 10)),
            None
        );
        assert_eq!(
            Extent::from_raw(10, 10).left_difference(&Extent::from_raw(10, 10)),
            None
        );
        assert_eq!(
            Extent::from_raw(10, 10).left_difference(&Extent::from_raw(20, 10)),
            Some(Extent::from_raw(10, 10))
        );

        assert_eq!(
            Extent::from_raw(10, 10).left_difference(&Extent::from_raw(5, 10)),
            None
        );
        assert_eq!(
            Extent::from_raw(10, 10).left_difference(&Extent::from_raw(15, 10)),
            Some(Extent::from_raw(10, 5))
        );

        assert_eq!(
            Extent::from_raw(10, 10).left_difference(&Extent::from_raw(12, 6)),
            Some(Extent::from_raw(10, 2))
        );

        assert_eq!(
            Extent::from_raw(10, 10).left_difference(&Extent::from_raw(8, 14)),
            None
        );
    }

    #[test]
    fn right_difference() {
        assert_eq!(
            Extent::from_raw(10, 10).right_difference(&Extent::from_raw(0, 10)),
            Some(Extent::from_raw(10, 10))
        );
        assert_eq!(
            Extent::from_raw(10, 10).right_difference(&Extent::from_raw(10, 10)),
            None
        );
        assert_eq!(
            Extent::from_raw(10, 10).right_difference(&Extent::from_raw(20, 10)),
            None
        );

        assert_eq!(
            Extent::from_raw(10, 10).right_difference(&Extent::from_raw(5, 10)),
            Some(Extent::from_raw(15, 5))
        );
        assert_eq!(
            Extent::from_raw(10, 10).right_difference(&Extent::from_raw(15, 10)),
            None
        );

        assert_eq!(
            Extent::from_raw(10, 10).right_difference(&Extent::from_raw(12, 6)),
            Some(Extent::from_raw(18, 2))
        );

        assert_eq!(
            Extent::from_raw(10, 10).right_difference(&Extent::from_raw(8, 14)),
            None
        );
    }
}
