#![no_std]
#![no_main]

use core::fmt::Write;
use core::iter::Iterator;
use core::panic::PanicInfo;

use static_assertions::assert_eq_size;

const VMEM: *mut u8 = 0xb8000 as *mut u8;

#[no_mangle]
pub extern "C" fn loader_main(boot_info_ptr: *const BootInfo) -> ! {
    // Assume `boot_info` is a valid pointer and that we won't overwrite it.
    let boot_info = unsafe { &*boot_info_ptr };

    clear_screen();

    let mut writer = ScreenWriter { offset: 0 };

    if !get_bit(boot_info.flags, BOOT_FLAGS_MMAP_BIT) {
        panic!("boot info has no memory map");
    }

    // Print the bootloader-provided memory map
    write!(&mut writer, "Memory map:").unwrap();

    let mmap_iter = unsafe { MemMapIter::from_boot_info(boot_info) };
    for entry in mmap_iter {
        write!(
            &mut writer,
            " ({}, {}, {})",
            entry.base_addr, entry.mem_length, entry.mem_type
        )
        .unwrap();
    }

    loop {}
}

// Multiboot information provided by bootloader
#[repr(C, packed)]
pub struct BootInfo {
    flags: u32,
    mem_lower: u32,
    mem_upper: u32,
    boot_device: u32,
    cmdline: u32,
    mods_count: u32,
    mods_addr: u32,
    syms: [u32; 4],
    mmap_length: u32,
    mmap_addr: u32,
    drives_length: u32,
    drives_addr: u32,
    config_table: u32,
    boot_loader_name: u32,
    apm_table: u32,
}

assert_eq_size!(BootInfo, [u8; 72]);

const BOOT_FLAGS_MODS_BIT: usize = 3;
const BOOT_FLAGS_MMAP_BIT: usize = 6;

fn get_bit(flags: u32, bit: usize) -> bool {
    assert!(bit < 32);
    (flags & (1 << bit)) > 0
}

#[repr(C, packed)]
struct MemMapEntryRaw {
    entry_size: u32,
    base_addr: u64,
    mem_length: u64,
    mem_type: u32,
}

struct MemMapEntry {
    base_addr: u64,
    mem_length: u64,
    mem_type: u32,
}

struct MemMapIter {
    next_entry: *const MemMapEntryRaw,
    length_remaining: usize,
}

impl MemMapIter {
    pub unsafe fn from_boot_info(boot_info: &BootInfo) -> MemMapIter {
        assert!(get_bit(boot_info.flags, BOOT_FLAGS_MMAP_BIT));
        MemMapIter {
            next_entry: boot_info.mmap_addr as *const MemMapEntryRaw,
            length_remaining: boot_info.mmap_length as usize,
        }
    }
}

impl Iterator for MemMapIter {
    type Item = MemMapEntry;

    fn next(&mut self) -> Option<Self::Item> {
        if self.length_remaining == 0 {
            return None;
        }

        // Each entry is at least 24 bytes
        assert!(self.length_remaining >= 24);

        let raw_entry = unsafe { &*self.next_entry };
        let entry_size = raw_entry.entry_size;
        if entry_size < 20 {
            panic!("entry size was {}, expected >= 20", entry_size);
        }

        self.next_entry =
            unsafe { (self.next_entry as *const u8).offset((raw_entry.entry_size + 4) as isize) }
                as *const MemMapEntryRaw;
        self.length_remaining -= (raw_entry.entry_size + 4) as usize;

        Some(MemMapEntry {
            base_addr: raw_entry.base_addr,
            mem_length: raw_entry.mem_length,
            mem_type: raw_entry.mem_type,
        })
    }
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

#[panic_handler]
fn panic(info: &PanicInfo<'_>) -> ! {
    clear_screen();

    let mut writer = ScreenWriter { offset: 0 };
    let _ = write!(&mut writer, "{}", info);

    loop {}
}
