// This implements a page frame allocator based on the buddy allocation scheme.

pub const FRAME_SIZE: usize = 4096;

// Physical memory map.
#[derive(Clone, Copy, Debug)]
struct MemoryMapEntry {
    addr: u64,
    size: u64,
}

static mut MEMORY_MAP: [MemoryMapEntry; 128] = [MemoryMapEntry{0, 0}; 128];

// We will store a memory bitmap above the kernel.
static mut BITMAP_ADDR: usize = 0;
static mut MEMORY_FRAMES: usize = 0;

fn kernel_image_bounds(mbinfo: &multiboot::Info) -> (usize, usize) {
    let symtab_info = multiboot::get_section_header_table_info(mbinfo);
    (0..symtab_info.entry_count)
        .map(|ndx| unsafe { elf::get_section_header(symtab_info.addr, symtab_info.entry_size, ndx) })
        .map(|header| (header.addr as usize, (header.addr + header.size) as usize))
        .filter(|&(lower, upper)| upper - lower > 0)
        .fold((size::max_value(), 0), |(a, b), (c, d)| (cmp::min(a, c), cmp::max(b, d)))
}

fn copy_memory_map(mbinfo: &multiboot::Info) {
    let mm_iterator = multiboot::get_memory_map_iterator(mbinfo)
        .map(|entry| MemoryMapEntry{addr: entry.base_addr, size: entry.length});

    let mut i = 0;
    for entry in mm_iterator {
        unsafe { MEMORY_MAP[i] = }
    }
}

pub fn init(mbinfo: &multiboot::Info) {

}
