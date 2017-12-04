use alloc::allocator::Alloc;
use alloc::allocator::Layout;

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
