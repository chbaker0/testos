use super::*;

use core::fmt::Write;
use core::panic::PanicInfo;

use lazy_static::lazy_static;
use log::{error, info};
use x86_64::instructions::interrupts;
use x86_64::structures::idt::InterruptStackFrame;

const VMEM: *mut u8 = 0xB8000 as *mut u8;

/// The kernel entry point. This is the `_start` symbol the loader `jmp`s to;
/// it is never called as an ordinary Rust function.
///
/// # Safety
///
/// `boot_info` must point to a valid `BootInfo` living in identity-mapped
/// physical memory, per the loader's handoff contract (see
/// `loader/src/main.rs` and `shared::boot_info::BootInfo`). Must be entered
/// exactly once, as the first kernel code to run.
#[unsafe(export_name = "_start")]
pub unsafe extern "C" fn kernel_entry(boot_info: *const shared::boot_info::BootInfo) -> ! {
    // SAFETY: this is the very first kernel code to run after the loader's
    // `jmp` (see loader/src/main.rs), so port 0xe9 has not been touched by
    // anything else yet and no other `QemuDebugWriter` can exist concurrently
    // (single core, no other kernel code has run).
    let mut debugcon = unsafe { shared::log::QemuDebugWriter::new() };
    let _ = writeln!(debugcon, "In kernel_entry");

    init_logger();

    interrupts::disable();

    info!("In kernel");

    gdt::init();
    info!("Set up GDT");

    idt::init();
    info!("Set up IDT");

    // SAFETY: forwarded from this fn's contract — the loader places this in
    // identity-mapped physical memory and gives us its address in rdi, per
    // shared::boot_info::BootInfo's contract.
    let boot_info = unsafe { &*boot_info };

    info!("init_extent = {:?}", boot_info.init_module);

    mm::init(boot_info);
    info!("Initialized frame allocator");

    let init_extent = mm::phys_extent_to_virt(boot_info.init_module);
    // SAFETY: `boot_info.init_module` is `MemoryType::KernelLoad` in the
    // loader's memory map (see loader/src/main.rs's `translate_memory_map`),
    // which `is_ram_backed()`, so `mm::init` (just above) has already mapped
    // it read/write into the phys_map window `phys_extent_to_virt` resolves
    // against. No one else holds a reference into it at this point in boot.
    let init_elf = xmas_elf::ElfFile::new(unsafe { &*init_extent.as_slice() }).unwrap();

    info!("init sections:");
    for section in init_elf
        .section_iter()
        .flat_map(|s| s.get_name(&init_elf).ok())
    {
        info!("  {}", section);
    }

    // SAFETY: this is the first and only call to
    // `sched::init_kernel_main_thread`; the scheduler has not been
    // initialized yet (see that function's own `# Safety` contract) and
    // `kernel_entry` never returns, so there is no "old" stack state left
    // dangling by the stack switch inside it.
    unsafe {
        sched::init_kernel_main_thread(kernel_main);
    }
}

pub fn kernel_main() -> ! {
    info!("In kernel_main");

    // This should do nothing.
    sched::yield_current();

    // SAFETY: `pic::init`'s contract requires interrupts to be disabled
    // before it runs and permits enabling them once it returns; `kernel_entry`
    // disabled interrupts on entry and nothing has re-enabled them since, so
    // both calls satisfy that ordering.
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
            // SAFETY: port 0xe9 is the conventional QEMU debugcon port and
            // isn't used elsewhere; `VMEM` points at the VGA text buffer, and
            // this `lazy_static` is the only place a `VgaWriter` over `VMEM`
            // is constructed outside the panic handler's fallback path (which
            // only runs if this LOGGER is already locked, i.e. never
            // concurrently with this initializer).
            static ref LOGGER: LogTee<LogSink<QemuDebugWriter>, LogSink<VgaWriter>> = unsafe { LogTee(LogSink::new(QemuDebugWriter::new()), LogSink::new(VgaWriter::new(VMEM))) };
        }
    } else {
        use shared::log::LogSink;
        use shared::vga::VgaWriter;
        lazy_static! {
            // SAFETY: as above (the `qemu_debugcon` variant) but for `VMEM`
            // alone: `VMEM` points at the VGA text buffer, and this is the
            // only place a `VgaWriter` over it is constructed outside the
            // panic handler's fallback (which only runs while this LOGGER is
            // locked).
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
            // SAFETY: port 0xe9 is the conventional QEMU debugcon port; we
            // only reach this branch when LOGGER (which owns the only other
            // writer to it) is locked, so there's no concurrent writer.
            let mut writer = unsafe { shared::log::QemuDebugWriter::new() };
            let _ = write!(&mut writer, "{info}");
        }

        // SAFETY: `VMEM` points at the VGA text buffer; we only reach this
        // branch when LOGGER (which owns the only other `VgaWriter` over
        // `VMEM`) is locked, so there's no concurrent writer. A panic-path
        // write racing the locked LOGGER's own in-progress write can still
        // interleave garbled output, but not touch invalid memory.
        let mut writer = unsafe { shared::vga::VgaWriter::new(VMEM) };
        let _ = write!(&mut writer, "{info}");
    }
    // Fail the CI smoke test on any panic (no-op outside CI).
    crate::qemu::exit_qemu(crate::qemu::QemuExitCode::Failed);
    interrupts::disable();
    halt_loop();
}
