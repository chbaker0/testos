use super::*;

use core::fmt::Write;
use core::panic::PanicInfo;

use lazy_static::lazy_static;
use log::{error, info};
use x86_64::instructions::interrupts;
use x86_64::structures::idt::InterruptStackFrame;

const VMEM: *mut u8 = 0xB8000 as *mut u8;

#[unsafe(export_name = "_start")]
pub extern "C" fn kernel_entry(boot_info: *const shared::boot_info::BootInfo) -> ! {
    let mut debugcon = unsafe { shared::log::QemuDebugWriter::new() };
    let _ = writeln!(debugcon, "In kernel_entry");

    init_logger();

    interrupts::disable();

    info!("In kernel");

    gdt::init();
    info!("Set up GDT");

    idt::init();
    info!("Set up IDT");

    // SAFETY: the loader places this in identity-mapped physical memory and
    // gives us its address in rdi, per shared::boot_info::BootInfo's contract.
    let boot_info = unsafe { &*boot_info };

    info!("init_extent = {:?}", boot_info.init_module);

    mm::init(boot_info);
    info!("Initialized frame allocator");

    let init_extent = mm::phys_extent_to_virt(boot_info.init_module);
    let init_elf = xmas_elf::ElfFile::new(unsafe { &*init_extent.as_slice() }).unwrap();

    info!("init sections:");
    for section in init_elf
        .section_iter()
        .flat_map(|s| s.get_name(&init_elf).ok())
    {
        info!("  {}", section);
    }

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
    let vec: alloc::vec::Vec<u32> = (0..100).collect();
    let mut string = alloc::string::String::new();
    for i in vec.iter() {
        write!(&mut string, "{i} ").unwrap();
    }

    info!("{string}");

    // A clean boot has reached the end of kernel_main. Signal the CI smoke
    // test that we passed (no-op outside CI), then halt as before.
    crate::qemu::exit_qemu(crate::qemu::QemuExitCode::Success);

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

cfg_if::cfg_if! {
    if #[cfg(feature = "qemu_debugcon")] {
        use shared::log::{LogTee, LogSink, QemuDebugWriter};
        use shared::vga::VgaWriter;
        lazy_static! {
            static ref LOGGER: LogTee<LogSink<QemuDebugWriter>, LogSink<VgaWriter>> = unsafe { LogTee(LogSink::new(QemuDebugWriter::new()), LogSink::new(VgaWriter::new(VMEM))) };
        }
    } else {
        use shared::log::LogSink;
        use shared::vga::VgaWriter;
        lazy_static! {
            static ref LOGGER: LogSink<VgaWriter> = unsafe { LogSink::new(VgaWriter::new(VMEM)) };
        }
    }
}

fn init_logger() {
    log::set_logger(&*LOGGER).unwrap();
    log::set_max_level(log::LevelFilter::Info);
}

#[panic_handler]
fn panic(info: &PanicInfo<'_>) -> ! {
    use shared::log::LogExt;

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
    // Fail the CI smoke test on any panic (no-op outside CI).
    crate::qemu::exit_qemu(crate::qemu::QemuExitCode::Failed);
    interrupts::disable();
    halt_loop();
}
