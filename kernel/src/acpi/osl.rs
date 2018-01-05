use alloc::allocator::Alloc;
use alloc::allocator::Layout;
use alloc::boxed::Box;
use core::mem;
use core::ptr::null_mut;
use spin;

use mm;
use sync;

type ACPI_STATUS = u32;

const AE_CODE_ENVIRONMENTAL: u32 = 0x0000;
const AE_CODE_PROGRAMMER: u32 = 0x1000;

const AE_OK: ACPI_STATUS = 0x0000;
const AE_BAD_PARAMETER: ACPI_STATUS = 0x0001 | AE_CODE_PROGRAMMER;
const AE_NO_MEMORY: ACPI_STATUS = 0x0004 | AE_CODE_ENVIRONMENTAL;

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

#[no_mangle]
pub extern "C" fn AcpiOsCreateLock(out_handle: *mut *mut spin::Mutex<()>) -> ACPI_STATUS {
    if out_handle == null_mut() {
        return AE_BAD_PARAMETER;
    } else {
        unsafe { *out_handle = Box::into_raw(Box::new(spin::Mutex::new(()))) };
        AE_OK
    }
}

#[no_mangle]
pub extern "C" fn AcpiOsDeleteLock(handle: *mut spin::Mutex<()>) {
    mem::drop(unsafe { Box::from_raw(handle) });
}

#[no_mangle]
pub extern "C" fn AcpiOsAcquireLock(handle: *mut spin::Mutex<()>) -> u64 {
    mem::forget(unsafe { (*handle).lock() });
    0
}

#[no_mangle]
pub extern "C" fn AcpiOsReleaseLock(handle: *mut spin::Mutex<()>, _: u64) {
    unsafe { (*handle).force_unlock(); }
}

#[no_mangle]
pub extern "C" fn AcpiOsCreateSemaphore(_max_units: u32, _initial_units: u32, _out_handle: *mut *mut sync::Semaphore) -> ACPI_STATUS {
    AE_NO_MEMORY
}

#[no_mangle]
pub extern "C" fn AcpiOsDeleteSemaphore(_handle: *mut sync::Semaphore) -> ACPI_STATUS {
    AE_BAD_PARAMETER
}

#[no_mangle]
pub extern "C" fn AcpiOsWaitSemaphore(_handle: *mut sync::Semaphore, _units: u32, _timeout: u16) -> ACPI_STATUS {
    AE_BAD_PARAMETER
}

#[no_mangle]
pub extern "C" fn AcpiOsSignalSemaphore(_handle: *mut sync::Semaphore, _units: u32) -> ACPI_STATUS {
    AE_BAD_PARAMETER
}
