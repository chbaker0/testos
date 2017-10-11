use shared::memory::*;

static mut MEMORY_MAP: MemoryMap = MemoryMap {
    entries: [MemoryMapEntry{base:0,length:0,status:MemoryStatus::Unknown}; MEMORY_MAP_MAX_ENTRIES],
    num_entries: 0,
};

// Must be initialized correctly in init function!
static mut FRAME_ALLOCATOR: FrameAllocator = FrameAllocator {
    cur_addr: 0,
    mem_map: unsafe { &MEMORY_MAP },
    cur_map_entry: 0
};

static mut INITIALIZED: bool = false;

// Public interface for initializing memory manager.
pub fn init(mem_map: MemoryMap) {
    if unsafe { INITIALIZED } {
        panic!("init called when already initialized.");
    }

    // Set up memory map.
    unsafe {
        MEMORY_MAP = mem_map;
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
