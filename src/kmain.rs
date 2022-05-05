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

    info!("Kernel base: {:X}", unsafe { &KERNEL_BASE as *const () }
        as usize);
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

fn translate_boot_info(mb2_info: &mb2::BootInformation) -> BootInfo {
    use shared::memory::*;
    let mem_map_tag = mb2_info.memory_map_tag().unwrap();
    let new_map = Map::from_entries(mem_map_tag.all_memory_areas().map(|area| MapEntry {
        extent: PhysExtent::from_raw(area.start_address(), area.size()),
        mem_type: match area.typ() {
            mb2::MemoryAreaType::Available => MemoryType::Available,
            mb2::MemoryAreaType::Reserved => MemoryType::Reserved,
            mb2::MemoryAreaType::AcpiAvailable => MemoryType::Acpi,
            mb2::MemoryAreaType::ReservedHibernate => MemoryType::ReservedPreserveOnHibernation,
            mb2::MemoryAreaType::Defective => MemoryType::Defective,
        },
    }));

    BootInfo {
        memory_map: new_map,
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

    // This points to nothing; it may only be used to construct a pointer.
    #[allow(improper_ctypes)]
    static KERNEL_BASE: ();
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
