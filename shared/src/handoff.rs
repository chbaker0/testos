use memory;

#[repr(C, packed)]
#[derive(Clone)]
pub struct BootInfo {
    pub mem_map: memory::MemoryMap,
}
