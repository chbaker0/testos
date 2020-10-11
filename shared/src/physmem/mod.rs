mod base;

use core::iter::IntoIterator;

use arrayvec::ArrayVec;
use itertools::put_back;
use itertools::structs::PutBack;

pub use base::*;

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
            block.address().offset_by(alloc_length),
            block.length().subtract(alloc_length),
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
        assert!(block.has_overlap(hole));
        let maybe_left = block.left_difference(hole);
        let maybe_right = block.right_difference(hole);

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

#[cfg(test)]
mod tests {
    use super::*;

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
