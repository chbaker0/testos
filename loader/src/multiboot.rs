use core::iter::Iterator;

use static_assertions::assert_eq_size;

use shared::memory;

// Multiboot information provided by bootloader
#[repr(C, packed)]
pub struct BootInfo {
    pub flags: u32,
    pub mem_lower: u32,
    pub mem_upper: u32,
    pub boot_device: u32,
    pub cmdline: u32,
    pub mods_count: u32,
    pub mods_addr: u32,
    pub syms: [u32; 4],
    pub mmap_length: u32,
    pub mmap_addr: u32,
    pub drives_length: u32,
    pub drives_addr: u32,
    pub config_table: u32,
    pub boot_loader_name: u32,
    pub apm_table: u32,
}

assert_eq_size!(BootInfo, [u8; 72]);

pub const BOOT_FLAGS_MODS_BIT: usize = 3;
pub const BOOT_FLAGS_MMAP_BIT: usize = 6;

pub unsafe fn parse_memory_map(boot_info: &BootInfo) -> memory::Map {
    assert!(get_bit(boot_info.flags, BOOT_FLAGS_MMAP_BIT));
    let iter = RawMemMapIter {
        next_entry: boot_info.mmap_addr as *const MemMapEntryRaw,
        length_remaining: boot_info.mmap_length as usize,
    };

    memory::Map::from_entries(iter.map(parse_memory_entry))
}

pub unsafe fn get_first_module(boot_info: &BootInfo) -> &'static [u8] {
    assert!(get_bit(boot_info.flags, BOOT_FLAGS_MODS_BIT));
    assert!(boot_info.mods_count > 0);

    let entry = &*(boot_info.mods_addr as *const ModEntry);
    core::slice::from_raw_parts(entry.start as *const u8, (entry.end - entry.start) as usize)
}

fn parse_memory_entry(raw: MemMapEntryRaw) -> memory::MapEntry {
    memory::MapEntry {
        extent: memory::Extent::new(
            memory::Address::from_raw(raw.base_addr),
            memory::Length::from_raw(raw.mem_length),
        ),
        mem_type: parse_memory_type(raw.mem_type),
    }
}

fn parse_memory_type(raw: u32) -> memory::MemoryType {
    use memory::MemoryType;

    match raw {
        1 => MemoryType::Available,
        3 => MemoryType::Acpi,
        4 => MemoryType::ReservedPreserveOnHibernation,
        5 => MemoryType::Defective,
        _ => MemoryType::Reserved,
    }
}

fn get_bit(flags: u32, bit: usize) -> bool {
    assert!(bit < 32);
    (flags & (1 << bit)) > 0
}

#[derive(Clone, Copy)]
#[repr(C, packed)]
struct MemMapEntryRaw {
    entry_size: u32,
    base_addr: u64,
    mem_length: u64,
    mem_type: u32,
}

struct RawMemMapIter {
    next_entry: *const MemMapEntryRaw,
    length_remaining: usize,
}

impl Iterator for RawMemMapIter {
    type Item = MemMapEntryRaw;

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

        Some(*raw_entry)
    }
}

#[repr(C, packed)]
struct ModEntry {
    start: u32,
    end: u32,
    string: u32,
    reserved: u32,
}
