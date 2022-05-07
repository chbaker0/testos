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

#[no_mangle]
pub extern "C" fn kernel_entry(mbinfo_addr: u64) -> ! {
    init_logger();

    info!("Multiboot info: {mbinfo_addr:X}");
    info!("{:X?}", *MB2_HEADER);

    let mbinfo = unsafe { mb2::load(mbinfo_addr as usize) }.unwrap();
    info!("{:?}", mbinfo);

    let boot_info = translate_boot_info(&mbinfo);

    interrupts::disable();

    info!("In kernel");

    gdt::init();
    info!("Set up GDT");

    idt::init();
    info!("Set up IDT");

    mm::init(&mbinfo);
    info!("Initialized frame allocator");

    unsafe {
        pic::init();
        interrupts::enable();
    }
    info!("Set up PIC");

    pic::install_irq_handler(1, Some(keyboard_handler));

    halt_loop();
}

fn translate_boot_info(mb2_info: &mb2::BootInformation) -> BootInfo {
    use shared::memory::*;
    BootInfo {
        memory_map: mm::translate_memory_map(mb2_info),
        // Fill these in with dummy values for now...
        kernel_extent: PhysExtent::from_raw(1024 * 1024, 1024 * 1024 * 6),
        boot_info_extent: PhysExtent::from_raw(0, 1),
        page_table_extent: PhysExtent::from_raw(0, 1),
    }
}

fn keyboard_handler(_: InterruptStackFrame) {
    panic!("keyboard interrupt received");
}

fn halt_loop() -> ! {
    loop {
        x86_64::instructions::hlt();
    }
}

extern "C" {
    // These point to valid memory, but they must not be dereferenced as is.
    static _binary_mb2_header_start: core::ffi::c_void;
    static _binary_mb2_header_end: core::ffi::c_void;
    static _binary_mb2_header_size: core::ffi::c_void;
}

#[used]
static MB2_HEADER_START: &core::ffi::c_void = unsafe { &_binary_mb2_header_start };
#[used]
static MB2_HEADER_END: &core::ffi::c_void = unsafe { &_binary_mb2_header_end };
#[used]
static MB2_HEADER_SIZE: &core::ffi::c_void = unsafe { &_binary_mb2_header_size };

lazy_static! {
    static ref MB2_HEADER: &'static [u8] = unsafe {
        core::slice::from_raw_parts(
            MB2_HEADER_START as *const _ as *const u8,
            MB2_HEADER_SIZE as *const _ as usize,
        )
    };
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
