use alloc::allocator::Alloc;
use alloc::allocator::Layout;

use mm;

#[no_mangle]
pub extern "C" fn AcpiOsPrintf(format: *const u8) {
    // Do nothing for now
}

#[repr(C, packed)]
struct AllocationHeader {
    size: u64,
}

#[no_mangle]
pub extern "C" fn AcpiOsAllocate(size: u64) -> *mut u8 {
    unsafe {
        let ptr = (&::ALLOCATOR).alloc(Layout::from_size_align(16 + size as usize, 16).unwrap()).unwrap();
        *(ptr as *mut AllocationHeader) = AllocationHeader { size: size };
        ptr.offset(16)
    }

}

#[no_mangle]
pub extern "C" fn AcpiOsFree(ptr: *mut u8) {
    unsafe {
        let nptr = ptr.offset(-16);
        let size = (*(nptr as *mut AllocationHeader)).size;
        (&::ALLOCATOR).dealloc(nptr, Layout::from_size_align(16 + size as usize, 16).unwrap());
    }
}

#[no_mangle]
pub extern "C" fn AcpiOsMapMemory(physical_address: u64, length: u64) -> *mut u8 {
    let first_frame = physical_address / mm::PAGE_SIZE as u64;
    let last_frame = (physical_address + length) / mm::PAGE_SIZE as u64;
    let num_pages = last_frame - first_frame + 1;
    let first_page = mm::allocate_address_space(num_pages).unwrap();
    for i in 0..num_pages {
        mm::map_to(mm::Page(first_page + i), mm::Frame(first_frame + i),
                   0, mm::get_frame_allocator());
    }

    let page_offset = physical_address - first_frame * mm::PAGE_SIZE as u64;
    (page_offset + first_page * mm::PAGE_SIZE as u64) as *mut u8
}

#[no_mangle]
pub extern "C" fn AcpiOsUnmapMemory(logical_address: u64, length: u64) {
    let first_page = logical_address / mm::PAGE_SIZE as u64;
    let last_page = (logical_address + length) / mm::PAGE_SIZE as u64;
    let num_pages = last_page - first_page + 1;
    for i in 0..num_pages {
        mm::unmap(mm::Page(first_page + i));
    }
    mm::deallocate_address_space(first_page * mm::PAGE_SIZE as u64, num_pages);
}
