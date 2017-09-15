use core::cmp;
use core::default::Default;
use multiboot;

pub const PAGE_ORDER: u32 = 12;
pub const PAGE_SIZE: usize = 1 << (PAGE_ORDER);

/* Memory map
 *
 * We store a map of available and reserved memory. There are a
 * maximum number of entries in this map, but it should be enough for
 * any sane configuration.
 */

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MemoryStatus {
    Unknown = 0, // Memory is unvailable because it is of unknown
                 // type.
    Available,   // Memory is available for use by the frame allocator.
    Kernel,      // Memory is unavailable because it is used by the
                 // kernel image.
    Reserved,    // Memory is unavailable because it is reserved.
}

#[derive(Clone, Copy)]
pub struct MemoryMapEntry {
    pub base: u64,
    pub length: u64,
    pub status: MemoryStatus,
}

impl Default for MemoryMapEntry {
    fn default() -> MemoryMapEntry {
        MemoryMapEntry {
            base: 0,
            length: 0,
            status: MemoryStatus::Unknown,
        }
    }
}

pub const MEMORY_MAP_MAX_ENTRIES: usize = 256;

pub struct MemoryMap {
    pub entries: [MemoryMapEntry; MEMORY_MAP_MAX_ENTRIES],
    pub num_entries: usize,
}

impl MemoryMap {
    pub fn from_multiboot(mbinfo: &multiboot::Info) -> Self {
        let mut memory_map = MemoryMap{entries:[Default::default(); MEMORY_MAP_MAX_ENTRIES], num_entries:0};

        let mut i = 0;
        for e in multiboot::get_memory_map_iterator(mbinfo) {
            memory_map.entries[i] = MemoryMapEntry {
                base: e.base_addr,
                length: e.length,
                status: MemoryStatus::Available,
            };
            i += 1;
        }
        memory_map.num_entries = i;

        memory_map
    }

    pub fn reserve(self: &mut Self, base: u64, length: u64) {
        assert!(length > 0);

        let mut o = 0;
        let mut new_entries = [Default::default(); MEMORY_MAP_MAX_ENTRIES];

        for e in self.entries.iter() {
            if e.base >= base && e.base + e.length <= base + length {
                // When the input range fully contains entry, delete entry.
            }
            else if e.base >= base + length || e.base + e.length <= base {
                // When the ranges are disjoint, copy original entry.
                new_entries[o] = *e;
                o += 1;
            }
            else {
                // Subtract input interval from entry interval.
                let left_base = e.base;
                let left_end = cmp::min(base, e.base + e.length);
                let right_base = cmp::max(base + length, e.base);
                let right_end = cmp::max(e.base + e.length, base + length);

                if left_end >= left_base {
                    new_entries[o] = MemoryMapEntry {
                        base: left_base,
                        length: left_end - left_base,
                        status: MemoryStatus::Available,
                    };
                    o += 1;
                }

                if right_end >= right_base {
                    new_entries[o] = MemoryMapEntry {
                        base: right_base,
                        length: right_end - right_base,
                        status: MemoryStatus::Available,
                    };
                    o += 1;
                }
            }
        }

        for i in 0..o {
            self.entries[i] = new_entries[i];
        }
        self.num_entries = o;
    }
}

/* Frame allocator
 *
 * This is a simple page frame allocator that hands out frames in
 * increasing order of address. We use the memory map to iterate over
 * usable memory.
 *
 * Idea from https://os.phil-opp.com/allocating-frames/
 */

// Fields are public as a bad hack; I need to initialize a
// FrameAllocator statically elsewhere.
pub struct FrameAllocator<'a> {
    pub cur_addr: usize,
    pub mem_map: &'a MemoryMap,
    pub cur_map_entry: usize,
}

fn align_address(address: u64, order: u32) -> u64 {
    let mask = (1 << order) - 1;
    let new_address = (address + mask) & !mask;
    assert!(new_address >= address);
    new_address
}


fn next_entry(mem_map: &MemoryMap, ndx: usize) -> usize {
    let mut i = ndx;
    while mem_map.entries[i].status != MemoryStatus::Available {
        i += 1;
    }
    i
}

impl<'a> FrameAllocator<'a> {
    pub fn new(mem_map: &'a MemoryMap) -> FrameAllocator<'a> {
        assert!(mem_map.num_entries > 0);
        let first = next_entry(&mem_map, 0);
        let entry = mem_map.entries[first];
        let base = align_address(entry.base, PAGE_ORDER);
        assert!(base <= usize::max_value() as u64);
        assert!(base < entry.base + entry.length);
        FrameAllocator {
            cur_addr: base as usize,
            mem_map: mem_map,
            cur_map_entry: first,
        }
    }

    pub fn get_frame(self: &mut Self) -> usize {
        let addr = self.cur_addr;
        self.cur_addr += PAGE_SIZE;

        let map_entry = self.mem_map.entries[self.cur_map_entry];
        let next_addr = self.cur_addr + PAGE_SIZE;
        if (next_addr as u64) > map_entry.base + map_entry.length {
            self.cur_map_entry = next_entry(&self.mem_map, self.cur_map_entry+1);
            if self.cur_map_entry >= self.mem_map.num_entries {
                panic!("Out of physical memory.");
            }
            let next_entry = self.mem_map.entries[self.cur_map_entry];
            let new_addr = align_address(next_entry.base, PAGE_ORDER);
            assert!(new_addr <= usize::max_value() as u64);
            assert!(new_addr < next_entry.base + next_entry.length);
            self.cur_addr = new_addr as usize;
        }

        addr
    }
}
