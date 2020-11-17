#![no_std]
#![no_main]

mod mm;

use core::fmt::Write;
use core::panic::PanicInfo;

use shared::handoff::BootInfo;
use shared::vga::VgaWriter;

const VMEM: *mut u8 = 0xB8000 as *mut u8;

#[no_mangle]
pub extern "C" fn kernel_entry(boot_info_addr: u64) -> ! {
    let mut writer = unsafe { VgaWriter::new(VMEM) };
    writeln!(&mut writer, "In kernel").unwrap();

    writeln!(&mut writer, "{:x}", boot_info_addr).unwrap();

    let boot_info = unsafe { (*(boot_info_addr as *const BootInfo)).clone() };
    writeln!(&mut writer, "{:?}", boot_info).unwrap();

    mm::init(&boot_info);
    writeln!(&mut writer, "Initialized frame allocator").unwrap();

    loop {}
}

#[panic_handler]
fn panic(info: &PanicInfo<'_>) -> ! {
    let mut writer = unsafe { VgaWriter::new(VMEM) };
    let _ = write!(&mut writer, "{}", info);
    loop {}
}
