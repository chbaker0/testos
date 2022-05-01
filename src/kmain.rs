use super::*;

use core::fmt::Write;
use core::panic::PanicInfo;
use lazy_static::lazy_static;
use log::info;
use multiboot2 as mb2;
use x86_64::instructions::interrupts;
use x86_64::structures::idt::InterruptStackFrame;

use shared::handoff::BootInfo;

const VMEM: *mut u8 = 0xB8000 as *mut u8;

#[export_name = "_start"]
pub extern "C" fn kernel_entry(mbinfo_addr: u64) -> ! {
    init_logger();

    let mbinfo = unsafe { mb2::load(mbinfo_addr as usize) }.unwrap();
    info!("{:?}", boot_info);
    halt_loop();

    interrupts::disable();

    info!("In kernel");

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
