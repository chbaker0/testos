#![no_main]
#![no_std]

use core::panic::PanicInfo;

#[export_name = "_start"]
pub extern "C" fn start() -> ! {
    loop {}
}

#[panic_handler]
fn panic(_info: &PanicInfo<'_>) -> ! {
    loop {}
}
