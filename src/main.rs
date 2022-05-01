#![deny(unsafe_op_in_unsafe_fn)]
#![feature(abi_x86_interrupt)]
#![no_std]
#![no_main]

mod gdt;
mod idt;
mod mm;
mod pic;

use core::fmt::Write;
use core::panic::PanicInfo;
use lazy_static::lazy_static;
use log::info;
use x86_64::instructions::interrupts;
use x86_64::structures::idt::InterruptStackFrame;

use shared::handoff::BootInfo;

const VMEM: *mut u8 = 0xB8000 as *mut u8;

#[no_mangle]
pub extern "C" fn kernel_entry(boot_info_addr: u64) -> ! {
    interrupts::disable();

    init_logger();
    info!("In kernel");

    info!("{:x}", boot_info_addr);

    let boot_info = unsafe { (*(boot_info_addr as *const BootInfo)).clone() };
    info!("{:?}", boot_info);

    gdt::init();
    info!("Set up GDT");

    idt::init();
    info!("Set up IDT");

    mm::init(&boot_info);
    info!("Initialized frame allocator");

    unsafe {
        pic::init();
        interrupts::enable();
    }
    info!("Set up PIC");

    pic::install_irq_handler(1, Some(keyboard_handler));

    halt_loop();
}

fn keyboard_handler(_: InterruptStackFrame) {
    panic!("keyboard interrupt received");
}

fn halt_loop() -> ! {
    loop {
        x86_64::instructions::hlt();
    }
}

lazy_static! {
    static ref LOGGER: shared::vga::VgaLog =
        shared::vga::VgaLog::new(unsafe { shared::vga::VgaWriter::new(VMEM) });
}

fn init_logger() {
    log::set_logger(&*LOGGER).unwrap();
    log::set_max_level(log::LevelFilter::Info);
}

#[panic_handler]
fn panic(info: &PanicInfo<'_>) -> ! {
    let mut writer = unsafe { shared::vga::VgaWriter::new(VMEM) };
    let _ = write!(&mut writer, "{}", info);
    interrupts::disable();
    halt_loop();
}
