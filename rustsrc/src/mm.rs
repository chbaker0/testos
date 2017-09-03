// This implements a page frame allocator based on the buddy allocation scheme.

pub const FRAME_SIZE: usize = 4096;
const MAX_ORDER: usize = 11; // Max block size of 4 MiB

// This data will be stored at the beginning of a free block. The
// blocks will form a linked list; they should be sorted in order of
// ascending physical address.
#[repr(C)]
struct FreeBlockHeader {
    next: usize, // Physical address of the next free block.
}

#[derive(Clone, Copy)]
struct FreeArea {
    head: usize,
}

static mut FREE_AREAS: [FreeArea; MAX_ORDER as usize] = [FreeArea{head: 0}; MAX_ORDER as usize];
static mut INITIALIZED: bool = false;

pub fn init(base_addr: usize, memory_size: usize) {
    if unsafe { INITIALIZED } {
        panic!("init called when memory management is already initialized.");
    }

    assert!(base_addr != 0);

    let min_alignment = FRAME_SIZE * (1 as usize).rotate_right((MAX_ORDER-1) as u32);
    assert!(0 == base_addr % min_alignment);
    assert!(0 == memory_size % min_alignment);

    unsafe {
        *(base_addr as *mut FreeBlockHeader) = FreeBlockHeader {next: 0};

        FREE_AREAS[MAX_ORDER-1] = FreeArea {head: base_addr};
    }
}
