//! Kernel memory management

pub mod paging;

use log::info;
pub use shared::memory::addr::*;
pub use shared::memory::page::*;

use shared::memory::alloc::*;
use shared::memory::*;

use goblin::elf64;
use multiboot2 as mb2;

// The maximum amount of memory the physical memory allocator supports. Exactly
// 128 GiB. TODO: remove this limit.
const MAX_MEMORY_BYTES: usize = 137438953472;

// The maximum number of frames the physical memory allocator supports. TODO: remove this limit.
const MAX_MEMORY_FRAMES: usize = MAX_MEMORY_BYTES / page::PAGE_SIZE.as_raw() as usize;

static FRAME_ALLOCATOR: spin::Mutex<once_cell::unsync::OnceCell<BitmapFrameAllocator>> =
    spin::Mutex::new(once_cell::unsync::OnceCell::new());

// Bitmap used by FRAME_ALLOCATOR. It is static to be allocated on kernel load,
// but it doesn't need to be; for example, if there were a simpler bootstrap
// allocator that didn't need a bitmap, the bitmap's memory could be allocated
// there.
//
// In fact, that is probably the better solution since that avoids memory
// limits. However, this suffices for now. TODO: dynamically allocate the
// bitmap's storage.
static FRAME_BITMAP: spin::Mutex<[u8; MAX_MEMORY_FRAMES / 8]> =
    spin::Mutex::new([0; MAX_MEMORY_FRAMES / 8]);

static KERNEL_PAGE_TABLE: spin::Mutex<once_cell::unsync::OnceCell<paging::PageTable>> =
    spin::Mutex::new(once_cell::unsync::OnceCell::new());

/// Initializes the memory management system. Must only be called once; panics
/// otherwise.
pub fn init(boot_info: &mb2::BootInformation) {
    // Make sure we are only called once.
    static IS_INITIALIZED: core::sync::atomic::AtomicBool =
        core::sync::atomic::AtomicBool::new(false);
    assert!(!IS_INITIALIZED.swap(true, core::sync::atomic::Ordering::SeqCst));

    let kernel_extent = get_kernel_extent(boot_info);
    info!("Kernel extent: {kernel_extent:X?}");

    let mut frame_bitmap = FRAME_BITMAP.lock();
    fill_bitmap_from_map(&mut *frame_bitmap, &translate_memory_map(&boot_info));

    // 'Leak' the reference `frame_bitmap`, leaving FRAME_BITMAP locked forever.
    // Now `frame_allocator` has exclusive access to the frame bitmap.
    let frame_bitmap_ref = spin::MutexGuard::leak(frame_bitmap);

    let mut frame_allocator = BitmapFrameAllocator::new(frame_bitmap_ref);

    // Mark all reserved areas. Important so we don't hand out memory containing
    // kernel code or data structures.
    for reserved_extent in [
        PhysExtent::from_raw(
            boot_info.start_address() as u64,
            boot_info.total_size() as u64,
        ),
        // boot_info.boot_info_extent,
        // boot_info.page_table_extent,
    ] {
        for frame in FrameRange::containing_extent(reserved_extent).iter() {
            frame_allocator.reserve(frame).unwrap();
        }
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

pub fn translate_memory_map(mb2_info: &mb2::BootInformation) -> Map {
    let mem_map_tag = mb2_info.memory_map_tag().unwrap();
    Map::from_entries(mem_map_tag.all_memory_areas().map(|area| MapEntry {
        extent: PhysExtent::from_raw(area.start_address(), area.size()),
        mem_type: match area.typ() {
            mb2::MemoryAreaType::Available => MemoryType::Available,
            mb2::MemoryAreaType::Reserved => MemoryType::Reserved,
            mb2::MemoryAreaType::AcpiAvailable => MemoryType::Acpi,
            mb2::MemoryAreaType::ReservedHibernate => MemoryType::ReservedPreserveOnHibernation,
            mb2::MemoryAreaType::Defective => MemoryType::Defective,
        },
    }))
}

fn get_kernel_extent(boot_info: &mb2::BootInformation) -> PhysExtent {
    let mut containing_extent: Option<PhysExtent> = None;
    for section in boot_info.elf_sections_tag().unwrap().sections() {
        if !section.is_allocated() || section.size() == 0 {
            continue;
        }
        assert_ne!(section.size(), 0, "{} {section:?}", section.name());
        let extent = PhysExtent::from_raw(section.start_address(), section.size());
        containing_extent = match containing_extent {
            Some(c) => Some(c.join(extent)),
            None => Some(extent),
        };
    }

    containing_extent.unwrap()
}
