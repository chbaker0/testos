#ifndef MULTIBOOT_H_INCLUDED_
#define MULTIBOOT_H_INCLUDED_

#include <stdint.h>

#define MULTIBOOT_INFO_FLAG_MEM (1U)
#define MULTIBOOT_INFO_FLAG_BOOT_DEVICE (2U)
#define MULTIBOOT_INFO_FLAG_CMDLINE (4U)
#define MULTIBOOT_INFO_FLAG_MODULES (8U)
#define MULTIBOOT_INFO_FLAG_AOUT_SYM (16U)
#define MULTIBOOT_INFO_FLAG_ELF_SYM (32U)
#define MULTIBOOT_INFO_FLAG_MMAP (64U)

struct multiboot_info
{
    uint32_t flags;
    // Size of upper and lower memory.
    uint32_t mem_lower;
    uint32_t mem_upper;
    // BIOS boot device.
    uint32_t boot_device;
    // Address of kernel command line string.
    uint32_t cmdline_addr;
    // Kernel module information.
    uint32_t mods_count;
    uint32_t mods_addr;
    // Kernel ELF section header table information.
    uint32_t shdr_num;
    uint32_t shdr_size;
    uint32_t shdr_addr;
    uint32_t shdr_shndx;
    // Memory map.
    uint32_t mmap_length;
    uint32_t mmap_addr;

    // TODO: Finish this struct.
};

#endif // MULTIBOOT_H_INCLUDED_
