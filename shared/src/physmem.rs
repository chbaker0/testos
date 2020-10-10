use core::cmp::{max, min};
use core::iter::IntoIterator;

use arrayvec::ArrayVec;
use itertools::put_back;
use itertools::structs::PutBack;

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
    pub fn align_down(&self, alignment: u64) -> Address {
        Self::from_raw(align_u64_down(self.as_raw(), alignment))
    }

    /// Returns the first address above `self` that is aligned to `alignment`,
    /// which must be a power of two.
    pub fn align_up(&self, alignment: u64) -> Address {
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
    pub fn align_down(&self, alignment: u64) -> Length {
        Length::from_raw(align_u64_down(self.as_raw(), alignment))
    }

    /// Returns the first length greater than `self` that is aligned to `alignment`,
    /// which must be a power of two.
    pub fn align_up(&self, alignment: u64) -> Length {
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
        Self::new_checked(address, length).unwrap()
    }

    pub fn new_checked(address: Address, length: Length) -> Option<Extent> {
        if length.as_raw() == 0 || length.as_raw() > u64::MAX - address.as_raw() {
            None
        } else {
            Some(Extent {
                address: address,
                length: length,
            })
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
    pub fn shrink_to_alignment(&self, alignment: u64) -> Option<Extent> {
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

    pub fn iter_type<'a>(&'a self, mem_type: MemoryType) -> impl Iterator<Item = Extent> + 'a {
        self.entries
            .iter()
            .filter(move |e| e.mem_type == mem_type)
            .map(|e| e.extent)
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
    // This is in reverse order.
    free: ArrayVec<[Extent; 128]>,
    // This is the base 2 log of the page size.
    page_size_log2: usize,
}

impl BumpAllocator {
    /// Allocates from the available memory in `map` after removing specified
    /// regions in `holes`. Allocations are multiples of `page_size`, which must
    /// be a power of two.
    ///
    /// `holes` must be sorted.
    ///
    /// Addresses in `map` and `holes` do not need to be aligned. However, all
    /// returned allocations will be aligned to `page_size`.
    pub fn new<T: IntoIterator<Item = Extent>>(
        page_size: u64,
        map: &Map,
        holes: T,
    ) -> BumpAllocator {
        // Make sure `page_size` is a power of two.
        assert_eq!(page_size.count_ones(), 1);

        // The base 2 log of a power of 2 is simply the number of trailing
        // zeros.
        let page_size_log2 = page_size.trailing_zeros() as usize;

        let map_iter = map.iter_type(MemoryType::Available);

        let mut free: ArrayVec<[Extent; 128]> = reserve(map_iter, holes)
            .flat_map(|e| e.shrink_to_alignment(page_size))
            .collect();
        free = free.iter().rev().copied().collect();

        BumpAllocator {
            free: free,
            page_size_log2: page_size_log2,
        }
    }

    pub fn allocate_pages(&mut self, pages: u64) -> Address {
        // Check that pages * (2^page_size_log2) <= u64::MAX without overflow.
        assert!(pages as u64 <= (u64::MAX >> self.page_size_log2));
        self.allocate_impl(Length::from_raw(pages << self.page_size_log2))
    }

    pub fn allocate(&mut self, length: Length) -> Address {
        self.allocate_impl(length.align_up(1 << self.page_size_log2))
    }

    // `alloc_length` must be aligned to the page size.
    fn allocate_impl(&mut self, alloc_length: Length) -> Address {
        assert!(alloc_length.as_raw().trailing_zeros() >= self.page_size_log2 as u32);

        // The last element of `self.free` contains the first available block.
        let mut block;
        loop {
            block = match self.free.pop() {
                Some(block) => block,
                None => panic!("out of memory"),
            };

            // Use this block if it's big enough.
            if alloc_length <= block.length() {
                break;
            }

            // Discard the block. We're just a bump allocator, after all.
        }

        let alloc_address = block.address();

        let maybe_remainder = Extent::new_checked(
            block.address().offset_by(&alloc_length),
            block.length().subtract(&alloc_length),
        );

        if let Some(remainder) = maybe_remainder {
            self.free.push(remainder);
        }

        alloc_address
    }
}

/// Removes specified regions from a list of blocks of memory.
///
/// Given `blocks`, a list of available memory, removes the regions specified in
/// `holes` and returns the remaining free memory. This may involve splitting
/// extents in `blocks`. The resulting list may be larger than `blocks`.
///
/// Both lists must be sorted by start address and non-overlapping.
fn reserve<T: IntoIterator<Item = Extent>, U: IntoIterator<Item = Extent>>(
    blocks: T,
    holes: U,
) -> impl Iterator<Item = Extent> {
    ReserveIter {
        blocks: put_back(blocks),
        holes: put_back(holes),
    }
    .flatten()
}

struct ReserveIter<I1: Iterator, I2: Iterator> {
    blocks: PutBack<I1>,
    holes: PutBack<I2>,
}

impl<I1, I2> Iterator for ReserveIter<I1, I2>
where
    I1: Iterator<Item = Extent>,
    I2: Iterator<Item = Extent>,
{
    type Item = Option<Extent>;

    fn next(&mut self) -> Option<Option<Extent>> {
        let block = self.blocks.next()?;

        // Remove holes completely before `ext`; they can be ignored.
        while let Some(hole) = self.holes.next() {
            if hole.last_address() >= block.address() {
                self.holes.put_back(hole);
                break;
            }
        }

        // Get the next hole. If there are none left, we can simply return
        // `block`.
        let hole = match self.holes.next() {
            Some(hole) => hole,
            None => return Some(Some(block)),
        };

        // If `hole` is completely after `block`, we can return `block`.
        // However, we must retain `hole` in case it intersects with a future
        // `block`.
        if block.last_address() < hole.address() {
            self.holes.put_back(hole);
            return Some(Some(block));
        }

        // We now know `hole` intersects `ext`: it is not completely before
        // `ext`, nor completely after `ext`. Get both sides of the
        // difference of `hole` from `ext`.
        assert!(block.has_overlap(&hole));
        let maybe_left = block.left_difference(&hole);
        let maybe_right = block.right_difference(&hole);

        if let Some(right) = maybe_right {
            // There may be another hole that will intersect `right`. Put it
            // back for next iteration. We can throw away `hole` though.
            self.blocks.put_back(right);
        } else {
            // `hole` may extend beyond `block`. Put it back for next iteration.
            self.holes.put_back(hole);
        }

        // There are no holes left that may intersect `maybe_left`, if it
        // exists. If `block` is none our caller will ignore the `Some(None)`
        // return value.
        Some(maybe_left)
    }
}

/// Given power-of-two `alignment`, returns the largest value below `x` aligned
/// to `alignment`
const fn align_u64_down(x: u64, alignment: u64) -> u64 {
    let mask = !(alignment - 1);
    x & mask
}

/// Given power-of-two `alignment`, returns the smallest value above `x` aligned
/// to `alignment`
const fn align_u64_up(x: u64, alignment: u64) -> u64 {
    align_u64_down(x + (alignment - 1), alignment)
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

    #[test]
    fn reserve_no_holes() {
        let result: Vec<_> = reserve(
            [Extent::from_raw(1, 4), Extent::from_raw(10, 4)]
                .iter()
                .copied(),
            [].iter().copied(),
        )
        .collect();
        assert_eq!(result, [Extent::from_raw(1, 4), Extent::from_raw(10, 4)]);

        let result: Vec<_> = reserve([].iter().copied(), [].iter().copied()).collect();
        assert_eq!(result, []);
    }

    #[test]
    fn reserve_no_blocks() {
        let result: Vec<_> = reserve(
            [].iter().copied(),
            [Extent::from_raw(5, 5), Extent::from_raw(15, 5)]
                .iter()
                .copied(),
        )
        .collect();
        assert_eq!(result, []);
    }

    #[test]
    fn reserve_big() {
        let result: Vec<_> = reserve(
            [
                Extent::from_raw(0, 5),
                Extent::from_raw(7, 2),
                Extent::from_raw(10, 10),
                Extent::from_raw(25, 5),
                Extent::from_raw(35, 10),
            ]
            .iter()
            .copied(),
            [
                Extent::from_raw(0, 3),
                Extent::from_raw(6, 4),
                Extent::from_raw(12, 4),
                Extent::from_raw(27, 3),
                Extent::from_raw(32, 4),
                Extent::from_raw(44, 2),
            ]
            .iter()
            .copied(),
        )
        .collect();

        assert_eq!(
            result,
            [
                Extent::from_raw(3, 2),
                Extent::from_raw(10, 2),
                Extent::from_raw(16, 4),
                Extent::from_raw(25, 2),
                Extent::from_raw(36, 8)
            ]
        );
    }

    #[test]
    fn reserve_multiple_holes_in_one_block() {
        let result: Vec<_> = reserve(
            [Extent::from_raw(10, 20)].iter().copied(),
            [
                Extent::from_raw(8, 4),
                Extent::from_raw(15, 5),
                Extent::from_raw(22, 2),
                Extent::from_raw(28, 10),
            ]
            .iter()
            .copied(),
        )
        .collect();

        assert_eq!(
            result,
            [
                Extent::from_raw(12, 3),
                Extent::from_raw(20, 2),
                Extent::from_raw(24, 4)
            ]
        );
    }

    #[test]
    fn reserve_multiple_blocks_in_one_hole() {
        let result: Vec<_> = reserve(
            [
                Extent::from_raw(4, 2),
                Extent::from_raw(10, 5),
                Extent::from_raw(20, 10),
                Extent::from_raw(38, 4),
            ]
            .iter()
            .copied(),
            [Extent::from_raw(5, 35)].iter().copied(),
        )
        .collect();

        assert_eq!(result, [Extent::from_raw(4, 1), Extent::from_raw(40, 2)]);
    }

    #[test]
    fn bump_allocator() {
        let page_size = 4096;

        let map = Map::from_entries(
            [MapEntry {
                extent: Extent::from_raw(0, 131072),
                mem_type: MemoryType::Available,
            }]
            .iter()
            .copied(),
        );

        let holes = [Extent::from_raw(4095, 4098)];

        let mut allocator = BumpAllocator::new(page_size, &map, holes.iter().copied());

        assert_eq!(
            allocator.allocate_pages(2),
            Address::from_raw(page_size * 3)
        );
        assert_eq!(
            allocator.allocate(Length::from_raw(20)),
            Address::from_raw(page_size * 5)
        );
        assert_eq!(
            allocator.allocate_pages(1),
            Address::from_raw(page_size * 6)
        );
    }
}
