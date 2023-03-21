pub mod addr;
pub mod alloc;
pub mod page;

use page::{FrameRange, PAGE_SIZE};

use core::iter::IntoIterator;

use arrayvec::ArrayVec;
use itertools::structs::PutBack;
use itertools::{put_back, Itertools};

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

        for entry in src.into_iter() {
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

    pub fn iter_type(&self, mem_type: MemoryType) -> impl Iterator<Item = MapEntry> + '_ {
        self.entries
            .iter()
            .filter(move |e| e.mem_type == mem_type)
            .copied()
    }
}

/// Given a sequence of memory regions, mark which areas contain kernel data
/// from another sequence of extents. Both sequences must be sorted and
/// non-overlapping.
///
/// Returns a sorted sequence of corrected regions.
pub fn mark_kernel_areas<T: IntoIterator<Item = MapEntry>, U: IntoIterator<Item = PhysExtent>>(
    regions: T,
    kernel_areas: U,
) -> impl Iterator<Item = MapEntry> {
    KernelAreaMarker {
        regions: put_back(regions),
        kernel_areas: put_back(kernel_areas),
    }
    .flatten()
}

/// Implementation of `mark_kernel_areas`. Ideally we'd have a generator
/// function but that's too unstable to rely on.
struct KernelAreaMarker<T: Iterator<Item = MapEntry>, U: Iterator<Item = PhysExtent>> {
    regions: PutBack<T>,
    kernel_areas: PutBack<U>,
}

impl<T: Iterator<Item = MapEntry>, U: Iterator<Item = PhysExtent>> Iterator
    for KernelAreaMarker<T, U>
{
    type Item = ArrayVec<MapEntry, 2>;

    fn next(&mut self) -> Option<Self::Item> {
        let cur = self.regions.next()?;

        let one_vec = |x| {
            let mut v = ArrayVec::new();
            v.push(x);
            v
        };

        // Some types we don't care about. Kernel extents shouldn't overlap with
        // anything other than `Available`, and in the off chance they do
        // there's nothing we can do.
        match cur.mem_type {
            MemoryType::Available => (),
            MemoryType::Acpi => (),
            _ => return Some(one_vec(cur)),
        }

        // Skip kernel pieces until we overlap the current region. This should
        // only be one, but just in case...
        while let Some(kernel) = self.kernel_areas.next() {
            if kernel.overlap(cur.extent).is_none() && kernel.address < cur.extent.address {
                continue;
            }

            self.kernel_areas.put_back(kernel);
            break;
        }

        // If there's no more kernel areas to consider, we just return the
        // region as is.
        let Some(kernel) = self.kernel_areas.next() else { return Some(one_vec(cur)) };

        // If this extent is completely after `cur`, we can return `cur`.
        // Put the extent back so we can consider it next round.
        if kernel.overlap(cur.extent).is_none() && kernel.address > cur.extent.address {
            self.kernel_areas.put_back(kernel);
            return Some(one_vec(cur));
        }

        // Otherwise, we have overlap and need to split. We return the left
        // difference and the kernel overlap, but not the right side which
        // we put back. We also put back the kernel extent. The right may
        // overlap the next kernel piece, and the next kernel piece might
        // overlap the next region.
        if let Some(right) = cur.extent.right_difference(kernel) {
            self.regions.put_back(MapEntry {
                extent: right,
                mem_type: cur.mem_type,
            })
        }

        let mut parts = ArrayVec::new();

        // Yield the left side (if it exists) and the kernel region.
        if let Some(left) = cur.extent.left_difference(kernel) {
            parts.push(MapEntry {
                extent: left,
                mem_type: cur.mem_type,
            });
        }

        parts.push(MapEntry {
            extent: cur.extent.overlap(kernel).unwrap(),
            mem_type: MemoryType::KernelLoad,
        });

        self.kernel_areas.put_back(kernel);

        Some(parts)
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

pub fn iter_map_frames<Iter: IntoIterator<Item = MapEntry>>(
    iter: Iter,
) -> impl Iterator<Item = FrameRange> {
    iter.into_iter()
        .map(|e| {
            Some(FrameRange::containing_extent(
                e.extent.shrink_to_alignment(PAGE_SIZE.as_raw())?,
            ))
        })
        .flatten()
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
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
    /// Available, but where the bootloader loaded us. Can't be used unless
    /// relocated.
    KernelLoad,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mark_kernel_areas() {
        let regions = [
            MapEntry {
                extent: PhysExtent::from_raw_range_exclusive(0, 100),
                mem_type: MemoryType::Available,
            },
            MapEntry {
                extent: PhysExtent::from_raw_range_exclusive(100, 200),
                mem_type: MemoryType::Reserved,
            },
            MapEntry {
                extent: PhysExtent::from_raw_range_exclusive(200, 300),
                mem_type: MemoryType::Available,
            },
            MapEntry {
                extent: PhysExtent::from_raw_range_exclusive(325, 375),
                mem_type: MemoryType::Defective,
            },
            MapEntry {
                extent: PhysExtent::from_raw_range_exclusive(400, 600),
                mem_type: MemoryType::Available,
            },
            MapEntry {
                extent: PhysExtent::from_raw_range_exclusive(700, 800),
                mem_type: MemoryType::Available,
            },
            MapEntry {
                extent: PhysExtent::from_raw_range_exclusive(800, 900),
                mem_type: MemoryType::Available,
            },
            MapEntry {
                extent: PhysExtent::from_raw_range_exclusive(900, 1000),
                mem_type: MemoryType::Available,
            },
            MapEntry {
                extent: PhysExtent::from_raw_range_exclusive(1100, 1200),
                mem_type: MemoryType::Available,
            },
        ];

        let areas = [
            PhysExtent::from_raw_range_exclusive(25, 75),
            PhysExtent::from_raw_range_exclusive(200, 300),
            PhysExtent::from_raw_range_exclusive(400, 500),
            PhysExtent::from_raw_range_exclusive(750, 775),
            PhysExtent::from_raw_range_exclusive(790, 825),
            PhysExtent::from_raw_range_exclusive(950, 1000),
        ];

        let correct = [
            MapEntry {
                extent: PhysExtent::from_raw_range_exclusive(0, 25),
                mem_type: MemoryType::Available,
            },
            MapEntry {
                extent: PhysExtent::from_raw_range_exclusive(25, 75),
                mem_type: MemoryType::KernelLoad,
            },
            MapEntry {
                extent: PhysExtent::from_raw_range_exclusive(75, 100),
                mem_type: MemoryType::Available,
            },
            MapEntry {
                extent: PhysExtent::from_raw_range_exclusive(100, 200),
                mem_type: MemoryType::Reserved,
            },
            MapEntry {
                extent: PhysExtent::from_raw_range_exclusive(200, 300),
                mem_type: MemoryType::KernelLoad,
            },
            MapEntry {
                extent: PhysExtent::from_raw_range_exclusive(325, 375),
                mem_type: MemoryType::Defective,
            },
            MapEntry {
                extent: PhysExtent::from_raw_range_exclusive(400, 500),
                mem_type: MemoryType::KernelLoad,
            },
            MapEntry {
                extent: PhysExtent::from_raw_range_exclusive(500, 600),
                mem_type: MemoryType::Available,
            },
            MapEntry {
                extent: PhysExtent::from_raw_range_exclusive(700, 750),
                mem_type: MemoryType::Available,
            },
            MapEntry {
                extent: PhysExtent::from_raw_range_exclusive(750, 775),
                mem_type: MemoryType::KernelLoad,
            },
            MapEntry {
                extent: PhysExtent::from_raw_range_exclusive(775, 790),
                mem_type: MemoryType::Available,
            },
            MapEntry {
                extent: PhysExtent::from_raw_range_exclusive(790, 800),
                mem_type: MemoryType::KernelLoad,
            },
            MapEntry {
                extent: PhysExtent::from_raw_range_exclusive(800, 825),
                mem_type: MemoryType::KernelLoad,
            },
            MapEntry {
                extent: PhysExtent::from_raw_range_exclusive(825, 900),
                mem_type: MemoryType::Available,
            },
            MapEntry {
                extent: PhysExtent::from_raw_range_exclusive(900, 950),
                mem_type: MemoryType::Available,
            },
            MapEntry {
                extent: PhysExtent::from_raw_range_exclusive(950, 1000),
                mem_type: MemoryType::KernelLoad,
            },
            MapEntry {
                extent: PhysExtent::from_raw_range_exclusive(1100, 1200),
                mem_type: MemoryType::Available,
            },
        ];

        pretty_assertions::assert_eq!(
            mark_kernel_areas(regions, areas).collect::<Vec<_>>(),
            correct.to_vec()
        );
    }
}
