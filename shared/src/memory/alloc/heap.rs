//! A simple heap allocator for arbitrary-sized allocations.

use intrusive_collections::singly_linked_list as sll;
use intrusive_collections::{intrusive_adapter, UnsafeRef};

/// Provides backing memory to `Heap`.
pub unsafe trait ChunkProvider<const CHUNK_SIZE: usize> {
    /// Allocate `num_chunks` contiguous chunks.
    fn allocate(num_chunks: usize) -> *mut [u8];
}

pub struct Heap<const CHUNK_SIZE: usize, Provider> {
    free_lists: [sll::SinglyLinkedList<BlockAdapter>; NUM_BLOCK_SIZES],
    provider: Provider,
}

impl<const CHUNK_SIZE: usize, Provider: ChunkProvider<CHUNK_SIZE>> Heap<CHUNK_SIZE, Provider> {
    pub fn new(provider: Provider) -> Self {
        Heap {
            free_lists: core::array::from_fn(|_| sll::SinglyLinkedList::new(BlockAdapter::new())),
            provider,
        }
    }
}

const NUM_BLOCK_SIZES: usize = 5;
const BLOCK_SIZES: [usize; NUM_BLOCK_SIZES] = [8, 16, 32, 64, 128];

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
#[repr(usize)]
enum BlockSizeKey {
    Size8 = 0,
    Size16 = 1,
    Size32 = 2,
    Size64 = 3,
    Size128 = 4,
}

impl BlockSizeKey {
    const fn size(self) -> usize {
        BLOCK_SIZES[self as usize]
    }
}

#[repr(C)]
struct FreeBlockData {
    size: BlockSizeKey,
    link: sll::AtomicLink,
}

intrusive_adapter!(BlockAdapter = UnsafeRef<FreeBlockData>: FreeBlockData { link: sll::AtomicLink });
