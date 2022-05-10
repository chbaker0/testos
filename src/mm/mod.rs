//! Kernel memory management

pub mod paging;

pub use shared::memory::addr::*;
pub use shared::memory::page::*;

use shared::memory::alloc::*;
use shared::memory::*;

use paging::*;

use goblin::elf64;
use log::info;
use multiboot2 as mb2;
use x86_64::registers::control::{Cr3, Cr3Flags};

// The maximum amount of memory the physical memory allocator supports. Exactly
// 128 GiB. TODO: remove this limit.
const MAX_MEMORY_BYTES: usize = 137438953472;
const MAX_MEMORY: Length = Length::from_raw(MAX_MEMORY_BYTES as u64);

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

static KERNEL_PAGE_TABLE: spin::Mutex<paging::PageTable> =
    spin::Mutex::new(paging::PageTable::zero());

/// Initializes the memory management system. Must only be called once; panics
/// otherwise.
pub fn init(boot_info: &mb2::BootInformation) {
    // Make sure we are only called once.
    static IS_INITIALIZED: core::sync::atomic::AtomicBool =
        core::sync::atomic::AtomicBool::new(false);
    assert!(!IS_INITIALIZED.swap(true, core::sync::atomic::Ordering::SeqCst));

    let kernel_extent = get_kernel_phys_extent();
    info!("Kernel extent: {kernel_extent:X?}");

    let memory_map = translate_memory_map(&boot_info);
    let mut frame_bitmap = FRAME_BITMAP.lock();
    fill_bitmap_from_map(&mut *frame_bitmap, &memory_map);

    // 'Leak' the reference `frame_bitmap`, leaving FRAME_BITMAP locked forever.
    // Now `frame_allocator` has exclusive access to the frame bitmap.
    let frame_bitmap_ref = spin::MutexGuard::leak(frame_bitmap);

    let mut frame_allocator = BitmapFrameAllocator::new(frame_bitmap_ref);

    // Mark all reserved areas. Important so we don't hand out memory containing
    // kernel code or data structures.
    for reserved_extent in [
        // Exclude the kernel image itself.
        get_kernel_phys_extent(),
        // Exclude the boot_info structure.
        PhysExtent::from_raw(
            boot_info.start_address() as u64,
            boot_info.total_size() as u64,
        ),
        // Exclude the first MB.
        PhysExtent::from_raw(0, 1024 * 1024),
    ] {
        for frame in FrameRange::containing_extent(reserved_extent).iter() {
            // Ignore if the frame isn't available. TODO: investigate why
            // unwrapping fails.
            let _ = frame_allocator.reserve(frame);
        }
    }

    FRAME_ALLOCATOR.lock().set(frame_allocator).unwrap();

    unsafe {
        set_up_initial_page_table(boot_info, &memory_map);
    }
}

#[allow(unused)]
pub fn allocate_frame() -> Frame {
    let mut guard = FRAME_ALLOCATOR.lock();
    let frame_allocator = guard.get_mut().unwrap();
    let frame = frame_allocator.allocate().unwrap();
    frame
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

unsafe fn set_up_initial_page_table(boot_info: &mb2::BootInformation, memory_map: &Map) {
    // Our bootstrap page table identity maps the first GB of memory.
    let first_gb_translator = |phys: PhysAddress| {
        assert!(phys.as_raw() < 1024 * 1024 * 1024, "{phys:?}");
        Some(VirtAddress::from_raw(phys.as_raw()))
    };

    let mut root_table = KERNEL_PAGE_TABLE.lock();
    // SAFETY:
    // * `root_table` is an empty page table, so all addresses are valid.
    // * `first_gb_translator` provides valid translations as long as the
    //   bootstrap page tables are in place.
    // * `allocate_frame` returns valid frames with our memory map in place.
    // * `root_table` is not yet the active page table.
    let mut mapper = unsafe {
        paging::Mapper::new(&mut root_table, first_gb_translator, || {
            Some(allocate_frame())
        })
    };

    let present_writable_nx =
        PageTableFlags::PRESENT | PageTableFlags::WRITABLE | PageTableFlags::EXECUTE_DISABLE;

    // Identity map first GB.
    for i in (0..1024 * 1024 * 1024).step_by(PAGE_SIZE.as_raw() as usize) {
        let page = Page::new(VirtAddress::from_raw(i));
        let frame = Frame::new(PhysAddress::from_raw(i));
        unsafe {
            mapper.map(page, frame, present_writable_nx).unwrap();
        }
    }

    // Map all phys memory at `PHYSICAL_MEMORY_MAP_OFFSET`.
    for entry in memory_map.entries() {
        let frames = FrameRange::containing_extent(entry.extent);
        for frame in frames.iter() {
            let phys = frame.start();
            let virt = phys_to_virt(phys);
            let page = Page::new(virt);
            unsafe {
                mapper.map(page, frame, present_writable_nx).unwrap();
            }
        }
    }

    // Map kernel.
    for section in boot_info.elf_sections_tag().unwrap().sections() {
        match section.section_type() {
            mb2::ElfSectionType::ProgramSection | mb2::ElfSectionType::Uninitialized => {
                info!("Mapping {section:?}");
            }
            _ => continue,
        }

        let mut page_flags = PageTableFlags::empty();
        if !section.flags().contains(mb2::ElfSectionFlags::ALLOCATED) {
            info!("Not allocated...");
            continue;
        }
        if section.flags().contains(mb2::ElfSectionFlags::WRITABLE) {
            page_flags |= PageTableFlags::WRITABLE;
        }
        if !section.flags().contains(mb2::ElfSectionFlags::EXECUTABLE) {
            page_flags |= PageTableFlags::EXECUTE_DISABLE;
        }

        for page in PageRange::containing_extent(VirtExtent::from_raw(
            section.start_address(),
            section.size(),
        ))
        .iter()
        {
            let frame = Frame::new(PhysAddress::from_zero(
                page.start() - get_kernel_virt_base(),
            ));
            unsafe {
                mapper.map(page, frame, page_flags).unwrap();
            }
        }
    }

    unsafe {
        install_page_table(&mut root_table);
    }
}

/// Install `root_table` as the active page table.
///
/// # Safety
/// * Must be a root PML4 table.
/// * Must correctly map the kernel's address space.
unsafe fn install_page_table(root_table: &mut paging::PageTable) {
    let phys_addr = kernel_ptr_to_phys_addr(root_table as *const _);
    unsafe {
        Cr3::write(
            x86_64::structures::paging::PhysFrame::from_start_address(x86_64::addr::PhysAddr::new(
                phys_addr.as_raw(),
            ))
            .unwrap(),
            Cr3Flags::empty(),
        );
    }
}

#[inline]
pub fn phys_to_virt(phys: PhysAddress) -> VirtAddress {
    assert!(phys < PhysAddress::from_zero(MAX_MEMORY));
    PHYSICAL_MEMORY_MAP_OFFSET + (phys - PhysAddress::zero())
}

/// All physical memory is linearly mapped starting here. The address is the
/// start of the higher half.
pub const PHYSICAL_MEMORY_MAP_OFFSET: VirtAddress = VirtAddress::from_raw(0xffff_8000_0000_0000);

/// Given a pointer `p` in the kernel's address space, return the physical
/// address referenced. `p` *must* point within the kernel's address space above
/// `get_kernel_virt_base()`.
#[inline]
pub fn kernel_ptr_to_phys_addr<T>(p: *const T) -> PhysAddress {
    let virt_addr = VirtAddress::from_ptr(p);
    assert!(virt_addr >= get_kernel_virt_base(), "{virt_addr:?}");
    PhysAddress::from_zero(virt_addr - get_kernel_virt_base())
}

#[inline]
pub fn get_kernel_virt_base() -> VirtAddress {
    // SAFETY: `KERNEL_VIRT_BASE` does not have a value, but it is zero-sized.
    // Its address is set appropriately by the linker so we may get a raw
    // pointers to it, as long as we never dereference it.
    unsafe { VirtAddress::from_raw(&internal::KERNEL_VIRT_BASE as *const _ as usize as u64) }
}

#[inline]
pub fn get_kernel_phys_extent() -> PhysExtent {
    // SAFETY: `KERNEL_PHYS_BEGIN_SYM` and `KERNEL_PHYS_END_SYM` do not have
    // values, but they zero-sized. The addresses are set appropriately by the
    // linker so we may get raw pointers to them, as long as we never
    // dereference them.
    unsafe {
        PhysExtent::from_raw_range_exclusive(
            &internal::KERNEL_PHYS_BEGIN_SYM as *const _ as usize as u64,
            &internal::KERNEL_PHYS_END_SYM as *const _ as usize as u64,
        )
    }
}

mod internal {
    extern "C" {
        #![allow(improper_ctypes)]
        // These may not be dereferenced. Only their address is meaningful.
        pub static KERNEL_PHYS_BEGIN_SYM: ();
        pub static KERNEL_PHYS_END_SYM: ();
        pub static KERNEL_VIRT_BASE: ();
    }
}
