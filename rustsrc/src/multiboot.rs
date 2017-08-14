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
