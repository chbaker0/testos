pub mod addr;
pub mod alloc;
pub mod page;
pub mod paging;

use page::{FrameRange, PAGE_SIZE};

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

    pub fn entries_mut(&mut self) -> &mut [MapEntry] {
        &mut self.entries[0..self.num_entries as usize]
    }

    /// Iterates the entries of the given type.
    ///
    /// Filters `entries()` — the `num_entries` prefix — not the whole backing
    /// array. `from_entries` fills the unused tail with dummy `Reserved`
    /// entries at address 0, so scanning the raw array would yield up to 128
    /// phantom one-byte extents to any caller asking for `Reserved`. That the
    /// only current caller asks for `Available` is what kept this invisible.
    pub fn iter_type(&self, mem_type: MemoryType) -> impl Iterator<Item = MapEntry> + '_ {
        self.entries()
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
        let Some(kernel) = self.kernel_areas.next() else {
            return Some(one_vec(cur));
        };

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
            });
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
    iter.into_iter().flat_map(|e| {
        Some(FrameRange::containing_extent(
            e.extent.shrink_to_alignment(PAGE_SIZE.as_raw())?,
        ))
    })
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

impl MemoryType {
    /// Whether this region is real RAM that the kernel should include in its
    /// identity map and physical-memory window (`phys_map`). Excludes
    /// address-space holes (MMIO and other `Reserved` ranges, which on real
    /// UEFI maps span multi-GiB regions) and unusable RAM.
    ///
    /// Used by both the loader's identity map and the kernel's `phys_map`
    /// extension so the two agree on which regions get mapped.
    pub const fn is_ram_backed(self) -> bool {
        match self {
            MemoryType::Available
            | MemoryType::Acpi
            | MemoryType::ReservedPreserveOnHibernation
            | MemoryType::KernelLoad => true,
            MemoryType::Reserved | MemoryType::Defective => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ram_backed_excludes_only_reserved_and_defective() {
        assert!(MemoryType::Available.is_ram_backed());
        assert!(MemoryType::Acpi.is_ram_backed());
        assert!(MemoryType::ReservedPreserveOnHibernation.is_ram_backed());
        assert!(MemoryType::KernelLoad.is_ram_backed());

        assert!(!MemoryType::Reserved.is_ram_backed());
        assert!(!MemoryType::Defective.is_ram_backed());
    }

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

#[cfg(kani)]
mod verify {
    //! Kani proof harnesses for [`crate::memory`]'s physical memory map.
    //!
    //! `Map` is the handoff structure the loader fills from the UEFI memory map
    //! and the kernel then trusts to decide which physical frames may be handed
    //! out. Its representation invariant — "only the first `num_entries` slots are
    //! real; the rest are dummies" — is enforced by convention rather than by the
    //! type, so every accessor has to respect it independently.

    use super::*;

    /// Number of real entries to model. `Map`'s backing array is 128 slots; three
    /// is enough to distinguish "reads only the real entries" from "reads the
    /// whole array", which is what these harnesses are about, without asking the
    /// solver to reason about 128 symbolic entries.
    const MODELLED_ENTRIES: usize = 3;

    fn any_mem_type() -> MemoryType {
        let choice: u8 = kani::any();
        kani::assume(choice < 6);
        match choice {
            0 => MemoryType::Available,
            1 => MemoryType::Acpi,
            2 => MemoryType::ReservedPreserveOnHibernation,
            3 => MemoryType::Defective,
            4 => MemoryType::Reserved,
            _ => MemoryType::KernelLoad,
        }
    }

    /// A `Map` with a symbolic number of entries (0..=3), each of a symbolic type.
    /// Extents are kept small and concrete: these harnesses are about *entry
    /// bookkeeping*, and the extent algebra is proved in `verify/addr.rs`.
    fn any_map() -> (Map, usize) {
        let count: usize = kani::any();
        kani::assume(count <= MODELLED_ENTRIES);

        let mut types = [MemoryType::Reserved; MODELLED_ENTRIES];
        for t in types.iter_mut() {
            *t = any_mem_type();
        }

        let map = Map::from_entries((0..count).map(|i| MapEntry {
            // Disjoint, ascending, non-empty — what `from_entries` documents as
            // its precondition.
            extent: PhysExtent::from_raw(i as u64 * 4096 + 4096, 4096),
            mem_type: types[i],
        }));

        (map, count)
    }

    /// `entries()` must expose exactly the entries that were inserted — no dummy
    /// tail, no truncation. Everything downstream (`mm::init`'s bootstrap-region
    /// search, `fill_bitmap_from_map`) iterates this.
    #[kani::proof]
    #[kani::unwind(5)]
    fn entries_exposes_exactly_the_inserted_entries() {
        let (map, count) = any_map();

        assert_eq!(map.entries().len(), count);
        for (i, e) in map.entries().iter().enumerate() {
            assert_eq!(e.extent.address().as_raw(), i as u64 * 4096 + 4096);
        }
    }

    /// `iter_type` must agree with filtering `entries()` by the same type,
    /// for *every* memory type — not just `Available`, the only one any
    /// caller asks for today.
    ///
    /// This harness used to fail: `iter_type` filtered `self.entries`, the
    /// whole 128-slot backing array, so it also yielded the dummy fill entries
    /// `from_entries` writes. Asking for `Reserved` returned up to 128 phantom
    /// one-byte extents at address 0. See `docs/kani-findings.md`.
    #[kani::proof]
    #[kani::unwind(8)]
    fn iter_type_matches_filtering_the_real_entries() {
        let (map, _count) = any_map();
        let wanted = any_mem_type();

        let via_iter_type = map.iter_type(wanted).count();
        let via_entries = map
            .entries()
            .iter()
            .filter(|e| e.mem_type == wanted)
            .count();

        assert_eq!(
            via_iter_type, via_entries,
            "iter_type must not see past num_entries into the dummy tail"
        );
    }

    /// `is_ram_backed` decides which regions get identity-mapped by the loader and
    /// windowed into `phys_map` by the kernel. The two sides must agree, so the
    /// classification has to be total and stable — proved over every variant
    /// rather than the six the unit test spells out.
    #[kani::proof]
    fn ram_backed_classification_is_total() {
        let t = any_mem_type();

        let backed = t.is_ram_backed();

        // Exactly the two unusable classes are excluded.
        let expected = !matches!(t, MemoryType::Reserved | MemoryType::Defective);
        assert_eq!(backed, expected);
    }

    /// `iter_map_frames` shrinks each entry to frame alignment and yields the
    /// frames strictly inside it. Anything it yields must lie within the original
    /// extent — a frame reaching outside would be memory the firmware never
    /// declared usable, marked free in the allocator bitmap.
    #[kani::proof]
    #[kani::unwind(4)]
    fn iter_map_frames_stays_within_its_entry() {
        let address: u64 = kani::any();
        let length: u64 = kani::any();
        kani::assume(length != 0);
        kani::assume(length <= u64::MAX - address);

        let entry = MapEntry {
            extent: PhysExtent::new(PhysAddress::from_raw(address), Length::from_raw(length)),
            mem_type: MemoryType::Available,
        };

        for range in iter_map_frames([entry]) {
            assert!(
                range.first().start().as_raw() >= address,
                "a yielded frame starts before the entry"
            );
            assert!(
                range.last().start().as_raw() + PAGE_SIZE.as_raw() - 1
                    <= entry.extent.last_address().as_raw(),
                "a yielded frame ends after the entry"
            );
        }
    }
}
