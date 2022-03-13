//! Kernel memory management

pub use shared::memory::addr::*;
pub use shared::memory::page::*;

use shared::memory::alloc::*;
use shared::memory::*;

// The maximum amount of memory the physical memory allocator supports. Exactly
// 128 GiB. TODO: remove this limit.
const MAX_MEMORY_BYTES: usize = 137438953472;

// The maximum number of frames the physical memory allocator supports. TODO: remove this limit.
const MAX_MEMORY_FRAMES: usize = MAX_MEMORY_BYTES / page::PAGE_SIZE.as_raw() as usize;

static FRAME_ALLOCATOR: spin::Mutex<once_cell::unsync::OnceCell<BitmapFrameAllocator>> =
    spin::Mutex::new(once_cell::unsync::OnceCell::new());

/// Initializes the memory management system. Must only be called once; panics
/// otherwise.
pub fn init(boot_info: &shared::handoff::BootInfo) {
    // Make sure we are only called once.
    static IS_INITIALIZED: core::sync::atomic::AtomicBool =
        core::sync::atomic::AtomicBool::new(false);
    assert!(!IS_INITIALIZED.swap(true, core::sync::atomic::Ordering::SeqCst));

    // Only one reference to this should ever exist. It is static to be
    // allocated on kernel load, but hypothetically it doesn't need to be; for
    // example, if there were a simpler bootstrap allocator that didn't need a
    // bitmap, the bitmap's memory could be allocated there.
    //
    // In fact, that is probably the better solution since that avoids memory
    // limits. However, this suffices for now. TODO: dynamically allocate the
    // bitmap's storage.
    static mut FRAME_BITMAP: [u8; MAX_MEMORY_FRAMES / 8] = [0; MAX_MEMORY_FRAMES / 8];

    // Get the *only* reference to FRAME_BITMAP.
    let frame_bitmap: &'static mut [u8] = unsafe { &mut FRAME_BITMAP };

    fill_bitmap_from_map(frame_bitmap, &boot_info.memory_map);

    // This holds the one and only reference to FRAME_BITMAP.
    let mut frame_allocator = unsafe { BitmapFrameAllocator::new(frame_bitmap) };

    // Mark all reserved areas
    for frame in [
        boot_info.kernel_extent,
        boot_info.boot_info_extent,
        boot_info.page_table_extent,
    ]
    .iter()
    .copied()
    .map(FrameRange::containing_extent)
    .flat_map(|r| r.iter())
    {
        frame_allocator.reserve(frame).unwrap();
    }

    FRAME_ALLOCATOR.lock().set(frame_allocator).unwrap();
}

#[allow(unused)]
pub fn allocate_frame() -> Frame {
    FRAME_ALLOCATOR
        .lock()
        .get_mut()
        .unwrap()
        .allocate()
        .unwrap()
}
