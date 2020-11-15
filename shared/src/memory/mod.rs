pub mod addr;
pub mod alloc;
pub mod page;

use core::iter::IntoIterator;

use arrayvec::ArrayVec;
use itertools::put_back;
use itertools::structs::PutBack;

pub use addr::*;

/// A map of the machine's physical memory.
#[derive(Clone)]
#[repr(C)]
pub struct Map {
    entries: [MapEntry; 128],
    num_entries: u64,
}

impl Map {
    /// `src` must be sorted by start address, and the extents must not overlap.
    pub fn from_entries<T: IntoIterator<Item = MapEntry>>(src: T) -> Map {
        // Create an array filled with meaningless dummy entries. We will
        // overwrite them with values from `src`.
        let mut entries = [MapEntry {
            extent: PhysExtent::from_raw(0, 1),
            mem_type: MemoryType::Reserved,
        }; 128];
        let mut num_entries: u64 = 0;

        let mut iter = src.into_iter();
        while let Some(entry) = iter.next() {
            assert!((num_entries as usize) < entries.len());
            entries[num_entries as usize] = entry;
            num_entries += 1;
        }

        Map {
            entries,
            num_entries,
        }
    }

    pub fn entries(&self) -> &[MapEntry] {
        &self.entries[0..self.num_entries as usize]
    }

    pub fn iter_type<'a>(&'a self, mem_type: MemoryType) -> impl Iterator<Item = PhysExtent> + 'a {
        self.entries
            .iter()
            .filter(move |e| e.mem_type == mem_type)
            .map(|e| e.extent)
    }
}

impl core::fmt::Debug for Map {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Map")
            .field("entries", &self.entries())
            .field("num_entries", &self.num_entries)
            .finish()
    }
}

#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct MapEntry {
    pub extent: PhysExtent,
    pub mem_type: MemoryType,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u64)]
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
    free: ArrayVec<[PhysExtent; 128]>,
    // This is the base 2 log of the page size.
    page_size_log2: usize,
}

impl BumpAllocator {
    /// Allocates from the listed Extents.
    ///
    /// `blocks` must be sorted. Addresses do not need to be aligned. However,
    /// all returned allocations will be aligned to `page_size`, which must be a
    /// power of two.
    pub fn new<T: IntoIterator<Item = PhysExtent>>(blocks: T, page_size: u64) -> BumpAllocator {
        // Make sure `page_size` is a power of two.
        assert_eq!(page_size.count_ones(), 1);

        // The base 2 log of a power of 2 is simply the number of trailing
        // zeros.
        let page_size_log2 = page_size.trailing_zeros() as usize;

        let mut free: ArrayVec<[PhysExtent; 128]> = blocks
            .into_iter()
            .flat_map(|e| e.shrink_to_alignment(page_size))
            .collect();

        assert!(is_sorted_and_nonoverlapping(free.iter().copied()));
        free = free.iter().rev().copied().collect();

        BumpAllocator {
            free,
            page_size_log2,
        }
    }

    /// Allocates from the available memory in `map` after removing specified
    /// regions in `holes`. Allocations are multiples of `page_size`, which must
    /// be a power of two.
    ///
    /// `holes` must be sorted.
    ///
    /// Addresses in `map` and `holes` do not need to be aligned. However, all
    /// returned allocations will be aligned to `page_size`.
    pub fn from_memory_map<T: IntoIterator<Item = PhysExtent>>(
        page_size: u64,
        map: &Map,
        holes: T,
    ) -> BumpAllocator {
        let map_iter = map.iter_type(MemoryType::Available);
        Self::new(remove_reserved(map_iter, holes), page_size)
    }

    pub fn page_size(&self) -> u64 {
        1 << self.page_size_log2
    }

    pub fn allocate_pages(&mut self, pages: u64) -> PhysExtent {
        // Check that pages * (2^page_size_log2) <= u64::MAX without overflow.
        assert!(pages as u64 <= (u64::MAX >> self.page_size_log2));
        let length = Length::from_raw(pages << self.page_size_log2);

        PhysExtent::new(
            self.allocate_impl(Length::from_raw(pages << self.page_size_log2)),
            length,
        )
    }

    pub fn allocate(&mut self, length: Length) -> PhysExtent {
        PhysExtent::new(
            self.allocate_impl(length.align_up(1 << self.page_size_log2)),
            length,
        )
    }

    // `alloc_length` must be aligned to the page size.
    fn allocate_impl(&mut self, alloc_length: Length) -> PhysAddress {
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

/// Removes specified regions from a list of memory blocks.
///
/// Given `blocks`, a list of available memory, removes the regions specified in
/// `holes` and returns the remaining free memory. This may involve splitting
/// extents in `blocks`. The resulting list may be larger than `blocks`.
///
/// Both lists must be sorted by start address and non-overlapping.
pub fn remove_reserved<T: IntoIterator<Item = PhysExtent>, U: IntoIterator<Item = PhysExtent>>(
    blocks: T,
    holes: U,
) -> impl Iterator<Item = PhysExtent> {
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
    I1: Iterator<Item = PhysExtent>,
    I2: Iterator<Item = PhysExtent>,
{
    type Item = Option<PhysExtent>;

    fn next(&mut self) -> Option<Option<PhysExtent>> {
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

pub fn is_sorted_and_nonoverlapping<
    AddrType: AddressType,
    T: IntoIterator<Item = Extent<AddrType>>,
>(
    blocks: T,
) -> bool {
    let mut iter = blocks.into_iter().peekable();

    while let Some(cur) = iter.next() {
        let next = match iter.peek().copied() {
            Some(next) => next,
            None => return true,
        };

        if cur.address() >= next.address() {
            return false;
        }

        if cur.has_overlap(next) {
            return false;
        }
    }

    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn remove_reserved_no_holes() {
        let result: Vec<_> = remove_reserved(
            [Extent::from_raw(1, 4), Extent::from_raw(10, 4)]
                .iter()
                .copied(),
            [].iter().copied(),
        )
        .collect();
        assert_eq!(result, [Extent::from_raw(1, 4), Extent::from_raw(10, 4)]);

        let result: Vec<_> = remove_reserved([].iter().copied(), [].iter().copied()).collect();
        assert_eq!(result, []);
    }

    #[test]
    fn remove_reserved_no_blocks() {
        let result: Vec<_> = remove_reserved(
            [].iter().copied(),
            [Extent::from_raw(5, 5), Extent::from_raw(15, 5)]
                .iter()
                .copied(),
        )
        .collect();
        assert_eq!(result, []);
    }

    #[test]
    fn remove_reserved_big() {
        let result: Vec<_> = remove_reserved(
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
    fn remove_reserved_multiple_holes_in_one_block() {
        let result: Vec<_> = remove_reserved(
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
    fn remove_reserved_multiple_blocks_in_one_hole() {
        let result: Vec<_> = remove_reserved(
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

        let mut allocator = BumpAllocator::from_memory_map(page_size, &map, holes.iter().copied());

        assert_eq!(
            allocator.allocate_pages(2),
            PhysExtent::from_raw(page_size * 3, page_size * 2)
        );
        assert_eq!(
            allocator.allocate(Length::from_raw(20)),
            PhysExtent::from_raw(page_size * 5, 20)
        );
        assert_eq!(
            allocator.allocate_pages(1),
            PhysExtent::from_raw(page_size * 6, page_size * 1)
        );
    }
}
