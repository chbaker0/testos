//! Kernel memory management

pub mod paging;

pub use shared::memory::addr::*;
pub use shared::memory::page::*;

use shared::memory::alloc::*;
use shared::memory::*;

use paging::*;

use log::info;
use multiboot2 as mb2;
use x86_64::registers::control::{Cr3, Cr3Flags};

/// The map of virtual address space. Assigns different ranges to various
/// purposes.
pub struct VirtualMap;

impl VirtualMap {
    /// Range of all user virtual address space. This is the lower-half.
    pub const fn user() -> VirtExtent {
        VirtExtent::from_raw_range_exclusive(0x0000_0000_0000_0000, 0x0000_8000_0000_0000)
    }

    /// Mapping of all physical memory in kernel space. This is currently 2^40
    /// bytes worth.
    pub const fn phys_map() -> VirtExtent {
        VirtExtent::from_raw_range_exclusive(0xffff_8000_0000_0000, 0xffff_80ff_ffff_ffff)
    }

    /// Kernel image's address. This is the last 2GiB of memory.
    pub const fn kernel_image() -> VirtExtent {
        VirtExtent::from_raw_range_exclusive(0xffff_ffff_8000_0000, 0xffff_ffff_ffff_ffff)
    }
}

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

// The maximum amount of memory the physical memory allocator supports. Exactly
// 128 GiB. TODO: remove this limit.
const MAX_MEMORY: Length = Length::from_raw(137438953472u64);

// The maximum number of frames the physical memory allocator supports. TODO: remove this limit.
const MAX_MEMORY_FRAMES: usize = MAX_MEMORY.as_raw() as usize / page::PAGE_SIZE.as_raw() as usize;

/// Initializes the memory management system. Must only be called once; panics
/// otherwise.
pub fn init(boot_info: &mb2::BootInformation, reserved: impl Iterator<Item = PhysExtent>) {
    // Make sure we are only called once.
    static IS_INITIALIZED: core::sync::atomic::AtomicBool =
        core::sync::atomic::AtomicBool::new(false);
    assert!(!IS_INITIALIZED.swap(true, core::sync::atomic::Ordering::SeqCst));

    let kernel_extent = get_kernel_phys_extent();
    info!("Kernel extent: {kernel_extent:X?}");

    let memory_map = translate_memory_map(boot_info);
    let mut frame_bitmap = FRAME_BITMAP.lock();
    fill_bitmap_from_map(&mut *frame_bitmap, &memory_map);

    // 'Leak' the reference `frame_bitmap`, leaving FRAME_BITMAP locked forever.
    // Now `frame_allocator` has exclusive access to the frame bitmap.
    let frame_bitmap_ref = spin::MutexGuard::leak(frame_bitmap);

    let mut frame_allocator = BitmapFrameAllocator::new(frame_bitmap_ref);

    // Mark all reserved areas. Important so we don't hand out memory containing
    // kernel code or data structures.
    for reserved_extent in reserved.chain([
        // Exclude the kernel image itself.
        get_kernel_phys_extent(),
        // Exclude the boot_info structure.
        PhysExtent::from_raw(
            boot_info.start_address() as u64,
            boot_info.total_size() as u64,
        ),
        // Exclude the first MB.
        PhysExtent::from_raw(0, 1024 * 1024),
    ]) {
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

#[inline(never)]
pub fn allocate_frame() -> Option<Frame> {
    Some(allocate_frames(0)?.first())
}

#[inline(never)]
pub fn allocate_frames(order: usize) -> Option<FrameRange> {
    let mut guard = FRAME_ALLOCATOR.lock();
    let frame_allocator = guard.get_mut().unwrap();
    frame_allocator.allocate_range(order)
}

#[inline(never)]
pub unsafe fn deallocate_frames(frames: FrameRange) {
    let mut guard = FRAME_ALLOCATOR.lock();
    let frame_allocator = guard.get_mut().unwrap();
    frame_allocator.deallocate_range(frames);
}

#[inline(never)]
pub fn allocate_owned_frames(order: usize) -> Option<OwnedFrameRange> {
    Some(OwnedFrameRange {
        frames: allocate_frames(order)?,
    })
}

/// An exclusively owned frame range that will be deallocated on destruction.
pub struct OwnedFrameRange {
    frames: FrameRange,
}

impl OwnedFrameRange {
    pub fn frames(&self) -> FrameRange {
        self.frames
    }
}

impl Drop for OwnedFrameRange {
    fn drop(&mut self) {
        unsafe {
            deallocate_frames(self.frames);
        }
    }
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

    let mut root_table = INIT_PAGE_TABLE.lock();
    // SAFETY:
    // * `root_table` is an empty page table, so all addresses are valid.
    // * `first_gb_translator` provides valid translations as long as the
    //   bootstrap page tables are in place.
    // * `allocate_frame` returns valid frames with our memory map in place.
    // * `root_table` is not yet the active page table.
    let mut mapper =
        unsafe { paging::Mapper::new(&mut root_table, first_gb_translator, allocate_frame) };

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
        let section_type = section.section_type();
        let section_flags = section.flags();
        let section_extent = VirtExtent::from_raw(section.start_address(), section.size());

        // Filter sections that don't occupy address space.
        if !section_flags.contains(mb2::ElfSectionFlags::ALLOCATED) {
            info!("Not allocated...");
            continue;
        }

        // Filter lower-half sections, used for bootstrap.
        if section.name().starts_with(".bootstrap") {
            info!("Ignoring lower half section {}", section.name());
            continue;
        }

        // Confirm the section is in the area we expect.
        assert!(
            VirtualMap::kernel_image().contains(section_extent),
            "{}: {:x?} does not contain {:x?}",
            section.name(),
            VirtualMap::kernel_image(),
            section_extent
        );

        match section_type {
            mb2::ElfSectionType::ProgramSection | mb2::ElfSectionType::Uninitialized => {
                info!("Mapping {} at {:X?}", section.name(), section_extent);
            }
            _ => continue,
        }

        let mut page_flags = PageTableFlags::PRESENT;
        if section_flags.contains(mb2::ElfSectionFlags::WRITABLE) {
            page_flags |= PageTableFlags::WRITABLE;
        }
        if !section_flags.contains(mb2::ElfSectionFlags::EXECUTABLE) {
            page_flags |= PageTableFlags::EXECUTE_DISABLE;
        }

        for page in PageRange::containing_extent(section_extent).iter() {
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

/// Contains kernel mappings that are exist in all page tables.
static PAGE_TABLE_TEMPLATE: spin::Mutex<paging::PageTable> =
    spin::Mutex::new(paging::PageTable::zero());

static INIT_PAGE_TABLE: spin::Mutex<paging::PageTable> =
    spin::Mutex::new(paging::PageTable::zero());

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

/// Get a kernel space virtual address corresponding to a physical memory
/// adddress.
///
/// The address is suitable but not necessarily safe for dereferencing as a
/// pointer in kernel code. This is unsafe if aliasing rules are broken
/// including if `phys` refers to memory backing another virtual mapping.
/// Furthermore, the memory at `phys` must be safe to read/write (which may not
/// be true e.g. for memory-mapped IO addresses).
///
/// This can be safe if `phys` was allocated by `allocate_frames` and not
/// subsequently deallocated. Even so, care must be taken to ensure to use it
/// safely if it was shared with other users.
#[inline]
pub fn phys_to_virt(phys: PhysAddress) -> VirtAddress {
    assert!(phys < PhysAddress::from_zero(MAX_MEMORY));
    VirtualMap::phys_map().address() + (phys - PhysAddress::zero())
}

/// Get a kernel space virtual extent corresponding to a physical memory
/// extent.
///
/// The same safety considerations as for `phys_to_virt` apply.
#[inline]
pub fn phys_extent_to_virt(phys: PhysExtent) -> VirtExtent {
    VirtExtent::new(phys_to_virt(phys.address()), phys.length())
}

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

/// Provides "chunks" or pages to the heap implementation. This is very basic:
/// it simply grabs frames, calculates the offset into our mapping of phys mem,
/// and hands that pointer down.
///
/// TODO: manage this better. I'd like to set aside a portion of the kernel's
/// address space for the heap.
struct HeapProvider;

unsafe impl heap::ChunkProvider for HeapProvider {
    fn allocate(&mut self, num_chunks: usize) -> *mut [core::mem::MaybeUninit<u8>] {
        let mut guard = FRAME_ALLOCATOR.lock();
        let frame_alloc = guard.get_mut().unwrap();

        let num_frames = num_chunks.next_power_of_two();
        let order = num_frames.trailing_zeros() as usize;
        let frames = frame_alloc.allocate_range(order).unwrap();

        let ptr: *mut core::mem::MaybeUninit<u8> =
            phys_to_virt(frames.first().start()).as_mut_ptr();
        core::ptr::slice_from_raw_parts_mut(ptr, num_chunks * PAGE_SIZE.as_raw() as usize)
    }
}

#[global_allocator]
static GLOBAL_ALLOCATOR: heap::CheckedHeap<HeapProvider> =
    heap::CheckedHeap::new(heap::Heap::new(HeapProvider));

mod internal {
    extern "C" {
        #![allow(improper_ctypes)]
        // These may not be dereferenced. Only their address is meaningful.
        pub static KERNEL_PHYS_BEGIN_SYM: ();
        pub static KERNEL_PHYS_END_SYM: ();
        pub static KERNEL_VIRT_BASE: ();
    }
}
