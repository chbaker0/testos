#![no_std]
#![no_main]

mod multiboot;

use core::fmt::Write;
use core::panic::PanicInfo;

use static_assertions::const_assert_eq;
use x86_64::structures::paging;
use xmas_elf::program;
use xmas_elf::ElfFile;

use shared::memory;

const PAGE_SIZE: u64 = 4096;

const VMEM: *mut u8 = 0xb8000 as *mut u8;

#[no_mangle]
pub extern "C" fn loader_main(boot_info_ptr: *const multiboot::BootInfo) -> ! {
    // Assume `boot_info` is a valid pointer and that we won't overwrite it.
    let boot_info = unsafe { &*boot_info_ptr };

    clear_screen();

    let mut writer = ScreenWriter { offset: 0 };

    // Copy the memory map from multiboot structures to our own memory.

    let memory_map = unsafe { multiboot::parse_memory_map(boot_info) };

    // Print the memory map
    write!(&mut writer, "Memory map:").unwrap();
    for entry in memory_map.entries() {
        write!(
            &mut writer,
            " ({}, {}, {:?})",
            entry.extent.address.as_raw(),
            entry.extent.length.as_raw(),
            entry.mem_type
        )
        .unwrap();
    }

    // Assume we won't overwrite the module.
    let kernel_image = unsafe { multiboot::get_first_module(boot_info) };

    writeln!(&mut writer, "\n").unwrap();
    writeln!(&mut writer, "Kernel addr: {:p}", kernel_image.as_ptr()).unwrap();
    writeln!(&mut writer, "Kernel size: {}", kernel_image.len()).unwrap();

    let kernel_elf = ElfFile::new(kernel_image).unwrap();

    write!(&mut writer, "Kernel sections:").unwrap();
    for section in kernel_elf.section_iter() {
        write!(
            &mut writer,
            " {}",
            section.get_name(&kernel_elf).unwrap_or("<null>")
        )
        .unwrap();
    }

    writeln!(&mut writer, "").unwrap();

    // Get the regions of memory we want to preserve before allocating and
    // loading the kernel.
    let loader_extent = get_loader_extent();
    let kernel_extent = memory::PhysExtent {
        address: memory::PhysAddress::from_raw(kernel_image.as_ptr() as u64),
        length: memory::Length::from_raw(kernel_image.len() as u64),
    };

    writeln!(&mut writer, "Loader extent: {:?}", get_loader_extent()).unwrap();

    // Reserve the loader's current memory, the kernel image's memory, and the
    // 1st MiB.
    let mut reserved_extents = [
        memory::PhysExtent::from_raw(0, 1024 * 1024),
        loader_extent,
        kernel_extent,
    ];
    reserved_extents.sort_unstable_by_key(|e| e.address());

    let mut allocator = memory::BumpAllocator::from_memory_map(
        PAGE_SIZE,
        &memory_map,
        reserved_extents.iter().copied(),
    );

    // This is where we'll copy the kernel sections.
    let kernel_target = memory::PhysExtent {
        address: allocator.allocate(kernel_extent.length()),
        length: get_kernel_load_size(&kernel_elf),
    };

    writeln!(&mut writer, "Kernel load target: {:?}", kernel_target).unwrap();

    let total_memory_extent = memory::PhysExtent::from_range_exclusive(
        memory::PhysAddress::from_raw(0),
        memory_map.entries().last().unwrap().extent.end_address(),
    );

    // Get the number of frames we need to create the page tables. Add 1 for the
    // top level PML4 table.
    let page_table_frames =
        estimate_frames_to_map(kernel_target) + estimate_frames_to_map(total_memory_extent) + 1;

    writeln!(
        &mut writer,
        "Frames required for page tables: {}",
        page_table_frames
    )
    .unwrap();

    // This is where we'll put the tables.
    let page_table_extent = memory::PhysExtent::new(
        allocator.allocate_pages(page_table_frames),
        memory::Length::from_raw(page_table_frames * PAGE_SIZE),
    );

    let mut page_table_allocator =
        memory::BumpAllocator::new([page_table_extent].iter().copied(), PAGE_SIZE);

    let mut page_table = paging::PageTable::new();

    unsafe {
        load_and_map_kernel_segments(
            &mut page_table,
            &mut page_table_allocator,
            &kernel_elf,
            kernel_target,
        );
    }

    unsafe {
        map_physical_memory(&mut page_table, &mut page_table_allocator, &memory_map);
    }

    loop {}
}

// Writes a string directly to the framebuffer, up to the max 80*25 = 2000
// characters. Very unsafe.
struct ScreenWriter {
    offset: isize,
}

impl Write for ScreenWriter {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        for c in s.chars() {
            if self.offset >= 80 * 25 {
                return Err(core::fmt::Error);
            }

            if c == '\n' {
                self.offset = ((self.offset + 79) / 80) * 80;
                return Ok(());
            }

            let b = if c.is_ascii() { c as u8 } else { '?' as u8 };

            unsafe {
                *VMEM.offset(2 * self.offset) = b;
            }
            self.offset += 1;
        }

        Ok(())
    }
}

fn clear_screen() {
    for i in 0..(80 * 25) {
        unsafe {
            *VMEM.offset(2 * i) = ' ' as u8;
        }
    }
}

unsafe fn load_and_map_kernel_segments(
    page_table: &mut paging::PageTable,
    allocator: &mut memory::BumpAllocator,
    kernel_image: &ElfFile,
    target: memory::PhysExtent,
) {
    use core::ptr::slice_from_raw_parts_mut;
    use core::ptr::write_bytes;

    use program::SegmentData;
    use program::Type;

    // For simplicity, zero out all memory at `target`. Then we don't have to
    // zero out memory per-segment.
    write_bytes(
        phys_addr_as_ptr(target.address()),
        0,
        target.length().as_raw() as usize,
    );

    let mut cur_dest_addr = target.address();

    for pg_header in kernel_image.program_iter() {
        assert!(cur_dest_addr < target.end_address());

        let segment_data = match pg_header.get_type().unwrap() {
            Type::Null => continue,
            Type::Load => pg_header.get_data(kernel_image).unwrap(),
            unsupported => panic!("unsupported section {:?}", unsupported),
        };

        let segment_slice = match segment_data {
            SegmentData::Undefined(slice) => slice,
            _ => panic!("unsupported segment data"),
        };

        assert!(
            cur_dest_addr.offset_by(memory::Length::from_raw(segment_slice.len() as u64))
                <= target.end_address()
        );
        let load_slice =
            slice_from_raw_parts_mut(cur_dest_addr.as_raw() as *mut u8, segment_slice.len());
        (*load_slice).copy_from_slice(segment_slice);

        let segment_length =
            memory::Length::from_raw(pg_header.mem_size() as u64).align_up(PAGE_SIZE);
        let segment_extent = memory::PhysExtent::new(cur_dest_addr, segment_length);

        map_linear(
            page_table,
            allocator,
            segment_extent,
            memory::VirtAddress::from_raw(pg_header.virtual_addr()),
        );

        cur_dest_addr = cur_dest_addr.offset_by(segment_length);
    }
}

fn get_kernel_load_size(kernel_image: &ElfFile) -> memory::Length {
    use program::Type;

    let mut length = memory::Length::from_raw(0);

    for pg_header in kernel_image.program_iter() {
        let segment_size = match pg_header.get_type().unwrap() {
            Type::Null => 0,
            Type::Load => pg_header.mem_size(),
            unsupported => panic!("unsupported section {:?}", unsupported),
        };

        length = length.add(memory::Length::from_raw(segment_size).align_up(PAGE_SIZE));
    }

    length
}

/// Given a region of memory, estimates the number of frames required to map it
/// linearly in a page table. Doesn't account for the L4 table.
///
/// Returns an upper bound. It may require fewer frames.
fn estimate_frames_to_map(extent: memory::PhysExtent) -> u64 {
    let extent = extent.expand_to_alignment(PAGE_SIZE);

    // First, compute the number of level-1 entries required. This is simply the
    // length divided by the frame size. The length is already aligned.
    let l1_entries = extent.length().as_raw() / PAGE_SIZE;

    // Each entry is 8 bytes.
    let l1_size = l1_entries * 8;

    // Depending on the virtual address it's mapped to, the first and last
    // tables may be partially filled. All the rest will be completely filled.
    // If all are completely filled, we need `l1_size / PAGE_SIZE` frames. Add 2
    // to this to account for the first and last frames.
    let l1_frames = l1_size / PAGE_SIZE + 2;

    // We can apply the same logic to each level up.

    let l2_size = l1_frames * 8;
    let l2_frames = l2_size / PAGE_SIZE + 2;

    let l3_size = l2_frames * 8;
    let l3_frames = l3_size / PAGE_SIZE + 2;

    return l1_frames + l2_frames + l3_frames;
}

fn get_loader_extent() -> memory::PhysExtent {
    let begin_address = unsafe {
        memory::PhysAddress::from_raw((&_loader_start as *const core::ffi::c_void) as u64)
    };

    let end_address =
        unsafe { memory::PhysAddress::from_raw((&_loader_end as *const core::ffi::c_void) as u64) };

    memory::PhysExtent::new(begin_address, begin_address.distance_to(end_address))
}

/// Map all physical memory to the bottom of the higher half
///
/// The 48-bit virtual address space is split into two halves of size 2^47. One
/// extends up from 0x0000_0000_0000_0000, and one extends down from
/// 0xFFFF_FFFF_FFFF_FFFF.
///
/// The top half starts at 0xFFFF_FFFF_FFFF_FFFF - (2^47-1), or
/// 0xFFFF_8000_0000_0000. We map all physical memory starting here.
unsafe fn map_physical_memory(
    page_table: &mut paging::PageTable,
    allocator: &mut memory::BumpAllocator,
    mem_map: &memory::Map,
) {
    const HIGHER_HALF_START: u64 = 0u64.overflowing_sub(1 << 47).0;
    const_assert_eq!(HIGHER_HALF_START, 0xFFFF_8000_0000_0000);

    let addr_zero = memory::PhysAddress::from_raw(0);
    let length = mem_map
        .entries()
        .last()
        .unwrap()
        .extent
        .end_address()
        .distance_from(addr_zero);
    let all_memory_extent = memory::PhysExtent::new(addr_zero, length);

    map_linear(
        page_table,
        allocator,
        all_memory_extent,
        memory::VirtAddress::from_raw(HIGHER_HALF_START),
    );
}

unsafe fn map_linear(
    page_table: &mut paging::PageTable,
    bump_allocator: &mut memory::BumpAllocator,
    extent: memory::PhysExtent,
    offset: memory::VirtAddress,
) {
    use x86_64::addr::{PhysAddr, VirtAddr};
    use x86_64::structures::paging::Mapper;

    let phys_to_virt =
        |frame: paging::PhysFrame| frame.start_address().as_u64() as *mut paging::PageTable;

    let mut frame_allocator = FrameAllocatorAdapter { bump_allocator };
    let mut mapper = paging::MappedPageTable::new(page_table, phys_to_virt);

    assert!(extent.is_aligned_to(PAGE_SIZE));
    assert!(offset.is_aligned_to(PAGE_SIZE));

    let mut page_flags = paging::PageTableFlags::empty();
    page_flags.insert(paging::PageTableFlags::PRESENT);
    page_flags.insert(paging::PageTableFlags::WRITABLE);
    page_flags.insert(paging::PageTableFlags::GLOBAL);

    let num_pages = extent.length().as_raw() / PAGE_SIZE;
    for cur_page in 0..num_pages {
        let cur_distance = memory::Length::from_raw(cur_page * PAGE_SIZE);
        let target_page: paging::Page<paging::Size4KiB> = paging::Page::from_start_address(
            VirtAddr::new(offset.offset_by(cur_distance).as_raw()),
        )
        .unwrap();
        let frame = paging::PhysFrame::from_start_address(PhysAddr::new(
            extent.address().offset_by(cur_distance).as_raw(),
        ))
        .unwrap();

        mapper
            .map_to_with_table_flags(
                target_page,
                frame,
                page_flags,
                page_flags,
                &mut frame_allocator,
            )
            .unwrap()
            .ignore();
    }

    for cur_page in 0..num_pages {
        let cur_distance = memory::Length::from_raw(cur_page * PAGE_SIZE);
        let target_page: paging::Page<paging::Size4KiB> = paging::Page::from_start_address(
            VirtAddr::new(offset.offset_by(cur_distance).as_raw()),
        )
        .unwrap();
        let frame = paging::PhysFrame::from_start_address(PhysAddr::new(
            extent.address().offset_by(cur_distance).as_raw(),
        ))
        .unwrap();

        assert_eq!(mapper.translate_page(target_page).unwrap(), frame);
    }
}

struct FrameAllocatorAdapter<'a> {
    bump_allocator: &'a mut memory::BumpAllocator,
}

unsafe impl<'a> paging::FrameAllocator<paging::Size4KiB> for FrameAllocatorAdapter<'a> {
    fn allocate_frame(&mut self) -> Option<paging::PhysFrame<paging::Size4KiB>> {
        use x86_64::addr::PhysAddr;

        let start_address = self.bump_allocator.allocate_pages(1);
        Some(paging::PhysFrame::from_start_address(PhysAddr::new(start_address.as_raw())).unwrap())
    }
}

unsafe fn phys_addr_as_ptr(address: memory::PhysAddress) -> *mut u8 {
    address.as_raw() as *mut u8
}

// DO NOT ACCESS THESE
extern "C" {
    static _loader_start: core::ffi::c_void;
    static _loader_end: core::ffi::c_void;
}

#[panic_handler]
fn panic(info: &PanicInfo<'_>) -> ! {
    clear_screen();

    let mut writer = ScreenWriter { offset: 0 };
    let _ = write!(&mut writer, "{}", info);

    loop {}
}
