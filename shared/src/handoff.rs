//! Informational structures the loader hands to the kernel

use static_assertions::assert_eq_size;

use crate::memory;

/// Core structure that the loader passes to the kernel. Contains important
/// information about the system and memory layout.
///
/// This struct *must* have the same layout between 32 and 64-bit builds.
#[derive(Clone, Debug)]
#[repr(C)]
pub struct BootInfo {
    /// System-provided map of available and reserved memory. See following
    /// fields for important regions provided by the loader.
    pub memory_map: memory::Map,
    /// Range of physical memory where the kernel was loaded. This must be
    /// preserved unless the kernel copies itself.
    pub kernel_extent: memory::PhysExtent,
    /// Where this structure itself resides. The kernel must copy this structure
    /// before reclaiming this memory.
    pub boot_info_extent: memory::PhysExtent,
    /// Where the page tables reside. The kernel must preserve this region until
    /// it creates its own page tables.
    pub page_table_extent: memory::PhysExtent,
}

// By checking against a constant size, we can ensure `BootInfo` has the same
// size in 32 and 64-bit. While this doesn't necessarily ensure the same layout,
// it is close enough.
//
// Whenever the size of `BootInfo` changes an error message will be emitted with
// the new size. The size below can be replaced.
assert_eq_size!(BootInfo, [u8; 3128]);
