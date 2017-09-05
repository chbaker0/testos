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

// Public interface for initializing memory manager.
pub fn init(mbinfo: &::multiboot::Info) {
    let mut i = 0;
    for e in ::multiboot::get_memory_map_iterator(mbinfo) {
        unsafe {
            MEMORY_MAP.entries[i] = MemoryMapEntry {
                base: e.base_addr,
                length: e.length,
                status: MemoryStatus::Available,
            };
        }
        i += 1;
    }
    unsafe {
        MEMORY_MAP.num_entries = i;
    }
}
