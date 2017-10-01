use core::option::Option;

#[repr(C)]
pub struct Info {
    pub flags: u32,
    // Size of upper and lower memory.
    pub mem_lower: u32,
    pub mem_upper: u32,
    // BIOS boot device.
    pub boot_device: u32,
    // Address of kernel command line string.
    pub cmdline_addr: u32,
    // Kernel module information.
    pub mods_count: u32,
    pub mods_addr: u32,
    // Kernel ELF sectionheader table info.
    pub shdr_num: u32,
    pub shdr_size: u32,
    pub shdr_addr: u32,
    pub shdr_shndx: u32,
    // Memory map.
    pub mmap_length: u32,
    pub mmap_addr: u32,
}

pub const INFO_FLAG_MEM: u32 = 1;
pub const IFNO_FLAG_BOOT_DEVICE: u32 = 2;
pub const INFO_FLAG_CMDLINE: u32 = 4;
pub const INFO_FLAG_MODULES: u32 = 8;
pub const INFO_FLAG_AOUT_SYM: u32 = 16;
pub const INFO_FLAG_ELF_SYM: u32 = 32;
pub const INFO_FLAG_MMAP: u32 = 64;

#[repr(C, packed)]
struct MemoryMapEntryRaw {
    size: u32,
    base_addr: u64,
    length: u64,
    mem_type: u32,
}

pub struct AvailableMemoryEntry {
    pub base_addr: u64,
    pub length: u64,
}

pub struct MemoryMapIterator {
    base: u32,
    length: u32,
    cur: u32,
}

impl Iterator for MemoryMapIterator {
    type Item = AvailableMemoryEntry;
    fn next(&mut self) -> Option<Self::Item> {
        while self.cur - self.base < self.length {
            let entry = unsafe { &*(self.cur as *const MemoryMapEntryRaw) };
            self.cur += entry.size + 4;

            if entry.mem_type == 1 {
                return Some(AvailableMemoryEntry {
                    base_addr: entry.base_addr,
                    length: entry.length,
                });
            }
        }

        None
    }
}

pub fn get_memory_map_iterator(mbinfo: &Info) -> MemoryMapIterator {
    MemoryMapIterator {
        base: mbinfo.mmap_addr,
        length: mbinfo.mmap_length,
        cur: mbinfo.mmap_addr,
    }
}

pub struct SectionHeaderTableInfo {
    pub addr: *const u8,
    pub entry_size: usize,
    pub entry_count: usize,
    pub string_table_ndx: usize,
}

pub fn get_section_header_table_info(mbinfo: &Info) -> SectionHeaderTableInfo {
    SectionHeaderTableInfo {
        addr: mbinfo.shdr_addr as *const u8,
        entry_size: mbinfo.shdr_size as usize,
        entry_count: mbinfo.shdr_num as usize,
        string_table_ndx: mbinfo.shdr_shndx as usize,
    }
}
