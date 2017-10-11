mod physmem;

pub use self::physmem::get_frame_allocator;
pub use shared::memory::FrameAllocator;
pub use shared::memory::MemoryMap;

static mut INITIALIZED: bool = false;

// Public interface for initializing memory manager.
pub fn init(mem_map: MemoryMap) {
    if unsafe { INITIALIZED } {
        panic!("init called when already initialized.");
    }

    physmem::init(mem_map);

    unsafe {
        INITIALIZED = true;
    }
}
