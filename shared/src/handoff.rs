use memory;

#[repr(C, packed)]
pub struct BootInfo {
    pub mem_map_addr: u64,
}
