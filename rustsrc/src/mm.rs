// This implements a page frame allocator based on the buddy allocation scheme.

pub const FRAME_SIZE: usize = 4096;
const MAX_ORDER: u32 = 11; // Max block size of 4 MiB

// This data will be stored at the beginning of a free block. The
// blocks will form a linked list; they should be sorted in order of
// ascending physical address.
struct FreeBlockHeader {
    next: usize, // Physical address of the next free block.
}

#[derive(Clone, Copy)]
struct FreeArea {
    head: usize,
}

static mut FREE_AREAS: [FreeArea; MAX_ORDER as usize] = [FreeArea{head: 0}; MAX_ORDER as usize];
