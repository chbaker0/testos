#![no_std]
#![no_main]

use core::panic::PanicInfo;

const VMEM: *mut u8 = 0xb8000 as *mut u8;

#[no_mangle]
pub extern "C" fn loader_main() -> ! {
    // Clear the screen.
    for i in 0..(80*25) {
        unsafe {
            *VMEM.offset(2*i) = ' ' as u8;
        }
    }

    // Write something to indicate we have control.
    unsafe {
        *VMEM = 'B' as u8;
    }

    loop {}
}

#[panic_handler]
fn panic(_panic: &PanicInfo<'_>) -> ! {
    unsafe {
        *VMEM = 'P' as u8;
    }

    loop {}
}
