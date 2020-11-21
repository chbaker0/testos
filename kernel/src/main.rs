#![no_std]
#![no_main]
#![feature(abi_x86_interrupt)]

mod gdt;
mod idt;
mod mm;
mod pic;

use core::fmt::Write;
use core::panic::PanicInfo;
use x86_64::instructions::interrupts;
use x86_64::structures::idt::InterruptStackFrame;

use shared::handoff::BootInfo;
use shared::vga::VgaWriter;

const VMEM: *mut u8 = 0xB8000 as *mut u8;

#[no_mangle]
pub extern "C" fn kernel_entry(boot_info_addr: u64) -> ! {
    interrupts::disable();

    let mut writer = unsafe { VgaWriter::new(VMEM) };
    writeln!(&mut writer, "In kernel").unwrap();

    writeln!(&mut writer, "{:x}", boot_info_addr).unwrap();

    let boot_info = unsafe { (*(boot_info_addr as *const BootInfo)).clone() };
    writeln!(&mut writer, "{:?}", boot_info).unwrap();

    gdt::init();
    writeln!(&mut writer, "Set up GDT").unwrap();

    idt::init();
    writeln!(&mut writer, "Set up IDT").unwrap();

    mm::init(&boot_info);
    writeln!(&mut writer, "Initialized frame allocator").unwrap();

    unsafe {
        pic::init();
        interrupts::enable();
    }
    writeln!(&mut writer, "Set up PIC").unwrap();

    pic::install_irq_handler(1, Some(keyboard_handler));

    halt_loop();
}

fn keyboard_handler(_: &mut InterruptStackFrame) {
    panic!("keyboard interrupt received");
}

fn halt_loop() -> ! {
    loop {
        x86_64::instructions::hlt();
    }
}

#[panic_handler]
fn panic(info: &PanicInfo<'_>) -> ! {
    let mut writer = unsafe { VgaWriter::new(VMEM) };
    let _ = write!(&mut writer, "{}", info);
    interrupts::disable();
    halt_loop();
}
