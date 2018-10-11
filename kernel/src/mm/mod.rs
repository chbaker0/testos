mod heap;
pub mod paging;
mod physmem;
mod virtmem;

pub use self::heap::GlobalAllocator;
pub use self::paging::Frame;
pub use self::paging::Page;
pub use self::paging::PAGE_SIZE;
pub use self::paging::map_to;
pub use self::physmem::get_frame_allocator;
pub use shared::memory::FrameAllocator;
pub use shared::memory::MemoryMap;

use spin;

static mut INITIALIZED: bool = false;

lazy_static! {
    static ref KERNEL_ADDR_SPACE: spin::Mutex<virtmem::AddressSpace> = {
        let addr_space = virtmem::AddressSpace::new(0xffff_8000_0000_0, 0xffff_ffff_ffff_f);
        spin::Mutex::new(addr_space)
    };
}

// Virtual memory map:
//   0xffff_8000_0000_0000 - 0xffff_fe7f_ffff_ffff: Unreserved
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

    {
        let mut addr_space = KERNEL_ADDR_SPACE.lock();
        addr_space.reserve(0xffff_fe80_0000_0, 0xffff_feff_ffff_f);
        addr_space.reserve(0xffff_ff00_0000_0, 0xffff_ff7f_ffff_f);
        addr_space.reserve(0xffff_ffff_8000_0, 0xffff_ffff_ffff_f);

        info!("Kernel virtual memory map:");
        for r in addr_space.iter() {
            info!("    {:x} {:x}", r.first_addr.get(), r.last_addr.get());
        }
    }

    unsafe {
        INITIALIZED = true;
    }
}

pub fn allocate_address_space(num_pages: u64) -> Result<u64, ()> {
    let mut addr_space = KERNEL_ADDR_SPACE.lock();
    addr_space.allocate(num_pages)
}
