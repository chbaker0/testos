use super::*;

use core::fmt::Write;
use core::panic::PanicInfo;

use lazy_static::lazy_static;
use log::{error, info};
use multiboot2 as mb2;
use x86_64::instructions::interrupts;
use x86_64::structures::idt::InterruptStackFrame;

const VMEM: *mut u8 = 0xB8000 as *mut u8;

#[no_mangle]
pub extern "C" fn kernel_entry(mbinfo_addr: u64) -> ! {
    init_logger();

    info!("Multiboot info: {mbinfo_addr:X}");
    info!("{:X?}", *MB2_HEADER);

    let mbinfo = unsafe { mb2::load(mbinfo_addr as usize) }.unwrap();
    info!("{:?}", mbinfo);

    interrupts::disable();

    info!("In kernel");

    gdt::init();
    info!("Set up GDT");

    idt::init();
    info!("Set up IDT");

    mm::init(&mbinfo);
    info!("Initialized frame allocator");

    unsafe {
        sched::init_kernel_main_thread(kernel_main);
    }
}

pub fn kernel_main() -> ! {
    info!("In kernel_main");

    // This should do nothing.
    sched::yield_current();

    unsafe {
        pic::init();
        interrupts::enable();
    }
    info!("Set up PIC");

    pic::install_irq_handler(1, Some(keyboard_handler));

    sched::spawn_kthread(test_thread, 0);
    info!("kernel_main yield");
    sched::yield_current();
    info!("kernel_main yield");
    sched::yield_current();
    info!("kernel_main after yield");

    // Try to use our really basic allocator.
    let vec: alloc::vec::Vec<u32> = (0..100).into_iter().collect();
    let mut string = alloc::string::String::new();
    for i in vec.iter() {
        write!(&mut string, "{i} ").unwrap();
    }

    info!("{string}");

    halt_loop();
}

pub extern "C" fn test_thread(_context: usize) -> ! {
    info!("Test thread before yield");
    sched::yield_current();
    info!("Test thread after yield");
    sched::quit_current();
}

fn keyboard_handler(_: InterruptStackFrame) {
    panic!("keyboard interrupt received");
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
    use shared::log::LogSink;
    use shared::vga::VgaWriter;

    cfg_if::cfg_if! {
        if #[cfg(feature = "qemu_debugcon")] {
            use shared::log::{LogPipe, QemuDebugWriter};

            lazy_static! {
                static ref LOGGER: LogPipe<LogSink<QemuDebugWriter>, LogSink<VgaWriter>> = unsafe { LogPipe(LogSink::new(QemuDebugWriter::new()), LogSink::new(VgaWriter::new(VMEM))) };
            }
        } else {
            lazy_static! {
                static ref LOGGER: LogSink<VgaWriter> = unsafe { LogSink::new(VgaWriter::new(VMEM)) };
            }
        }
    }

    log::set_logger(&*LOGGER).unwrap();
    log::set_max_level(log::LevelFilter::Info);
}

#[panic_handler]
fn panic(info: &PanicInfo<'_>) -> ! {
    // It is unlikely that we panicked while our LOGGER instance was locked, and
    // if we were, we'll likely triple fault anyway. Try to use the existing
    // LOGGER, and otherwise try to use a new VgaWriter.
    if !LOGGER.is_locked() {
        error!("{info}");
    } else {
        #[cfg(feature = "qemu_debugcon")]
        {
            let mut writer = unsafe { shared::log::QemuDebugWriter::new() };
            let _ = write!(&mut writer, "{info}");
        }

        let mut writer = unsafe { shared::vga::VgaWriter::new(VMEM) };
        let _ = write!(&mut writer, "{info}");
    }
    interrupts::disable();
    halt_loop();
}
