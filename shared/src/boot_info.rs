//! Data the UEFI loader hands off to the kernel at entry.

use crate::memory::{Map, PhysAddress, PhysExtent};

/// Everything the kernel needs from the loader to bootstrap itself, before it
/// can query anything about the machine on its own.
///
/// The loader places this in identity-mapped physical memory and passes its
/// physical address to `kernel_entry`.
#[repr(C)]
pub struct BootInfo {
    /// The machine's physical memory map, as reported by UEFI after boot
    /// services were exited.
    pub memory_map: Map,
    /// Physical address of the page table the loader built and installed.
    /// It already maps the kernel image and identity-maps all physical
    /// memory the loader knew about; the kernel extends it in place rather
    /// than building a new one.
    pub page_table_root: PhysAddress,
    /// Physical extent of the raw `init` ELF, loaded but not parsed or
    /// mapped by the loader.
    pub init_module: PhysExtent,
    /// Physical extent spanning the kernel's loaded segments, as placed by
    /// the loader via `allocate_pages`. The kernel can't derive this from its
    /// own linker symbols — those describe virtual addresses, and the loader
    /// chooses the physical placement dynamically at boot.
    pub kernel_image: PhysExtent,
}
