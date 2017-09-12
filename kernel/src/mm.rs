use core::cmp;
use core::mem::uninitialized;
use shared::elf;
use shared::multiboot;

const PAGE_ORDER: u32 = 12;
pub const PAGE_SIZE: usize = 1 << (PAGE_ORDER);

/* Memory map
 *
 * We store a map of available and reserved memory. There are a
 * maximum number of entries in this map, but it should be enough for
 * any sane configuration.
 */

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
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


fn next_entry(mem_map: &MemoryMap, ndx: usize) -> usize {
    let mut i = ndx;
    while mem_map.entries[i].status != MemoryStatus::Available {
        i += 1;
    }
    i
}

impl<'a> FrameAllocator<'a> {
    fn new(mem_map: &'a MemoryMap) -> FrameAllocator<'a> {
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

// Must be initialized correctly in init function!
static mut FRAME_ALLOCATOR: FrameAllocator = FrameAllocator {
    cur_addr: 0,
    mem_map: unsafe { &MEMORY_MAP },
    cur_map_entry: 0
};

static mut INITIALIZED: bool = false;

// Public interface for initializing memory manager.
pub fn init(mbinfo: &multiboot::Info) {
    if unsafe { INITIALIZED } {
        panic!("init called when already initialized.");
    }

    let (_, kernel_end) = {
        let symtab_info = multiboot::get_section_header_table_info(mbinfo);
        (0..symtab_info.entry_count)
            .map(|ndx| unsafe { elf::get_section_header(symtab_info.addr, symtab_info.entry_size, ndx) })
            .map(|header| (header.addr as u64, (header.addr + header.size) as u64))
            .filter(|&(lower, upper)| upper - lower > 0)
            .fold((u64::max_value(), 0), |(a, b), (c, d)| (cmp::min(a, c), cmp::max(b, d)))
    };


    // Set up memory map.
    let memory_map = unsafe { &mut MEMORY_MAP };

    // We only consider memory above 1 MiB, and the first memory here will be the kernel.
    memory_map.entries[0] = MemoryMapEntry {
        base: 0x100000,
        length: kernel_end - 0x100000,
        status: MemoryStatus::Kernel,
    };

    let mut i = 1;
    for e in multiboot::get_memory_map_iterator(mbinfo) {
        if e.base_addr + e.length < kernel_end { continue; }

        let new_base = cmp::max(kernel_end, e.base_addr);
        let new_length = e.length - (new_base - e.base_addr);
        memory_map.entries[i] = MemoryMapEntry {
            base: new_base,
            length: new_length,
            status: MemoryStatus::Available,
        };
        i += 1;
    }
    memory_map.num_entries = i;

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
