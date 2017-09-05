use core::mem::uninitialized;

const PAGE_ORDER: u32 = 12;
pub const PAGE_SIZE: usize = 1 << (PAGE_ORDER);

/* Memory map
 *
 * We store a map of available and reserved memory. There are a
 * maximum number of entries in this map, but it should be enough for
 * any sane configuration.
 */

#[derive(Clone, Copy, Debug)]
enum MemoryStatus {
    Unknown = 0, // Memory is unvailable because it is of unknown
                 // type.
    Available,   // Memory is available for use by the frame allocator.
    Kernel,      // Memory is unavailable because it is used by the
                 // kernel image.
    Reserved,    // Memory is unavailable because it is reserved.
}

#[derive(Clone, Copy)]
struct MemoryMapEntry {
    base: u64,
    length: u64,
    status: MemoryStatus,
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

const MEMORY_MAP_MAX_ENTRIES: usize = 256;

struct MemoryMap {
    entries: [MemoryMapEntry; MEMORY_MAP_MAX_ENTRIES],
    num_entries: usize,
}

static mut MEMORY_MAP: MemoryMap = MemoryMap {
    entries: [MemoryMapEntry{base:0,length:0,status:MemoryStatus::Unknown}; MEMORY_MAP_MAX_ENTRIES],
    num_entries: 0,
};

/* Frame allocator
 *
 * This is a simple page frame allocator that hands out frames in
 * increasing order of address. We use the memory map to iterate over
 * usable memory.
 *
 * Idea from https://os.phil-opp.com/allocating-frames/
 */

pub struct FrameAllocator<'a> {
    cur_addr: usize,
    mem_map: &'a MemoryMap,
    cur_map_entry: usize,
}

fn align_address(address: u64, order: u32) -> u64 {
    let mask = (1 << order) - 1;
    let new_address = (address + mask) & !mask;
    assert!(new_address >= address);
    new_address
}

impl<'a> FrameAllocator<'a> {
    fn new(mem_map: &'a MemoryMap) -> FrameAllocator<'a> {
        assert!(mem_map.num_entries > 0);
        let first = mem_map.entries[0];
        let base = align_address(first.base, PAGE_ORDER);
        assert!(base <= usize::max_value() as u64);
        assert!(base < first.base + first.length);
        FrameAllocator {
            cur_addr: base as usize,
            mem_map: mem_map,
            cur_map_entry: 0,
        }
    }

    pub fn get_frame(self: &mut Self) -> usize {
        let addr = self.cur_addr;
        self.cur_addr += PAGE_SIZE;

        let map_entry = self.mem_map.entries[self.cur_map_entry];
        let next_addr = self.cur_addr + PAGE_SIZE;
        if (next_addr as u64) > map_entry.base + map_entry.length {
            self.cur_map_entry += 1;
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

// Must be initialized correctly in init function!
static mut FRAME_ALLOCATOR: FrameAllocator = FrameAllocator {
    cur_addr: 0,
    mem_map: unsafe { &MEMORY_MAP },
    cur_map_entry: 0
};

static mut INITIALIZED: bool = false;

// Public interface for initializing memory manager.
pub fn init(mbinfo: &::multiboot::Info) {
    if unsafe { INITIALIZED } {
        panic!("init called when already initialized.");
    }

    // Copy multiboot memory map.
    let mut i = 0;
    for e in ::multiboot::get_memory_map_iterator(mbinfo) {
        if e.base_addr >= 0x100000 {
            unsafe {
                MEMORY_MAP.entries[i] = MemoryMapEntry {
                    base: e.base_addr,
                    length: e.length,
                    status: MemoryStatus::Available,
                };
            }
            i += 1;
        }
    }
    unsafe {
        MEMORY_MAP.num_entries = i;
    }

    // Initialize frame allocator.
    unsafe {
        FRAME_ALLOCATOR = FrameAllocator::new(&MEMORY_MAP);
    }

    unsafe {
        INITIALIZED = true;
    }
}

// Public interface for frame allocations.
pub fn get_frame_allocator() -> &'static mut FrameAllocator<'static> {
    assert!(unsafe { INITIALIZED });
    // Currently, locking is unnecessary. When there are multiple
    // threads of execution locking must be added.
    unsafe { &mut FRAME_ALLOCATOR }
}
