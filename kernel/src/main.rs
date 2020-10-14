#![no_std]
#![no_main]

use core::panic::PanicInfo;

const VMEM: *mut u8 = 0xFFFF_8000_000B_8000 as *mut u8;

#[no_mangle]
pub extern "C" fn kernel_entry() -> ! {
    unsafe {
        *VMEM = 'K' as u8;
    }
    loop {}
}

#[panic_handler]
fn panic(_info: &PanicInfo<'_>) -> ! {
    loop {}
}
