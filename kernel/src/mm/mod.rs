mod heap;
mod paging;
mod physmem;

pub use self::heap::allocate_raw;
pub use self::heap::allocate;
pub use self::paging::Frame;
pub use self::paging::Page;
pub use self::paging::map_to;
pub use self::physmem::get_frame_allocator;
pub use shared::memory::FrameAllocator;
pub use shared::memory::MemoryMap;

static mut INITIALIZED: bool = false;

// Virtual memory map:
//   0xffff_fe80_0000_0000 - 0xffff_feff_ffff_ffff: Kernel heap
//   0xffff_ff00_0000_0000 - 0xffff_ff7f_ffff_ffff: Recursive page mapping
//   0xffff_ffff_8000_0000 - 0xffff_ffff_ffff_ffff: Kernel image

// Public interface for initializing memory manager.
pub fn init(mem_map: MemoryMap) {
    if unsafe { INITIALIZED } {
        panic!("init called when already initialized.");
    }

    physmem::init(mem_map);
    heap::init();

    unsafe {
        INITIALIZED = true;
    }
}
