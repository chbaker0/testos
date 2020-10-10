#![no_std]
#![no_main]

mod multiboot;

use core::fmt::Write;
use core::panic::PanicInfo;

use xmas_elf::ElfFile;

use shared::physmem;

const VMEM: *mut u8 = 0xb8000 as *mut u8;

#[no_mangle]
pub extern "C" fn loader_main(boot_info_ptr: *const multiboot::BootInfo) -> ! {
    // Assume `boot_info` is a valid pointer and that we won't overwrite it.
    let boot_info = unsafe { &*boot_info_ptr };

    clear_screen();

    let mut writer = ScreenWriter { offset: 0 };

    // Copy the memory map from multiboot structures to our own memory.

    let memory_map = unsafe { multiboot::parse_memory_map(boot_info) };

    // Print the memory map
    write!(&mut writer, "Memory map:").unwrap();
    for entry in memory_map.entries() {
        write!(
            &mut writer,
            " ({}, {}, {:?})",
            entry.extent.address.as_raw(),
            entry.extent.length.as_raw(),
            entry.mem_type
        )
        .unwrap();
    }

    // Assume we won't overwrite the module.
    let kernel_data = unsafe { multiboot::get_first_module(boot_info) };

    writeln!(&mut writer, "\n").unwrap();
    writeln!(&mut writer, "Kernel addr: {:p}", kernel_data.as_ptr()).unwrap();
    writeln!(&mut writer, "Kernel size: {}", kernel_data.len()).unwrap();

    let kernel_elf = ElfFile::new(kernel_data).unwrap();

    write!(&mut writer, "Kernel sections:").unwrap();
    for section in kernel_elf.section_iter() {
        write!(
            &mut writer,
            " {}",
            section.get_name(&kernel_elf).unwrap_or("<null>")
        )
        .unwrap();
    }

    writeln!(&mut writer, "").unwrap();

    // Get the regions of memory we want to preserve before allocating and
    // loading the kernel.
    let loader_extent = get_loader_extent();
    let kernel_extent = physmem::Extent {
        address: physmem::Address::from_raw(kernel_data.as_ptr() as u64),
        length: physmem::Length::from_raw(kernel_data.len() as u64),
    };

    writeln!(&mut writer, "Loader extent: {:?}", get_loader_extent()).unwrap();

    // Reserve the loader's current memory, the kernel image's memory, and the
    // 1st MiB.
    let mut reserved_extents = [
        physmem::Extent::from_raw(0, 1024 * 1024),
        loader_extent,
        kernel_extent,
    ];
    reserved_extents.sort_unstable_by_key(|e| e.address());

    let mut allocator =
        physmem::BumpAllocator::new(4096, &memory_map, reserved_extents.iter().copied());

    // This is where we'll copy the kernel sections.
    let kernel_target = physmem::Extent {
        address: allocator.allocate(kernel_extent.length()),
        length: kernel_extent.length(),
    };

    writeln!(&mut writer, "Kernel load target: {:?}", kernel_target).unwrap();

    loop {}
}

// Writes a string directly to the framebuffer, up to the max 80*25 = 2000
// characters. Very unsafe.
struct ScreenWriter {
    offset: isize,
}

impl Write for ScreenWriter {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        for c in s.chars() {
            if self.offset >= 80 * 25 {
                return Err(core::fmt::Error);
            }

            if c == '\n' {
                self.offset = ((self.offset + 79) / 80) * 80;
                return Ok(());
            }

            let b = if c.is_ascii() { c as u8 } else { '?' as u8 };

            unsafe {
                *VMEM.offset(2 * self.offset) = b;
            }
            self.offset += 1;
        }

        Ok(())
    }
}

fn clear_screen() {
    for i in 0..(80 * 25) {
        unsafe {
            *VMEM.offset(2 * i) = ' ' as u8;
        }
    }
}

fn get_loader_extent() -> physmem::Extent {
    let begin_address =
        unsafe { physmem::Address::from_raw((&_loader_start as *const core::ffi::c_void) as u64) };

    let end_address =
        unsafe { physmem::Address::from_raw((&_loader_end as *const core::ffi::c_void) as u64) };

    physmem::Extent::new(begin_address, begin_address.distance_to(&end_address))
}

// DO NOT ACCESS THESE
extern "C" {
    static _loader_start: core::ffi::c_void;
    static _loader_end: core::ffi::c_void;
}

#[panic_handler]
fn panic(info: &PanicInfo<'_>) -> ! {
    clear_screen();

    let mut writer = ScreenWriter { offset: 0 };
    let _ = write!(&mut writer, "{}", info);

    loop {}
}
