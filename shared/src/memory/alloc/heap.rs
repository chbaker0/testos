//! A simple heap allocator for arbitrary-sized allocations.

use core::char::MAX;
use core::mem::MaybeUninit;
use core::ptr::{addr_of, NonNull};

use intrusive_collections::{intrusive_adapter, UnsafeRef};
use intrusive_collections::{singly_linked_list as sll, Adapter};
use log::info;
use static_assertions::const_assert;

/// Provides backing memory to `Heap`. `CHUNK_SIZE` must be a power of 2.
pub unsafe trait ChunkProvider<
    const CHUNK_SIZE: usize = { crate::memory::page::PAGE_SIZE.as_raw() as usize },
>
{
    /// Allocate `num_chunks` contiguous chunks.
    ///
    /// # Safety
    ///
    /// The implementation must return a valid slice sized and aligned to
    /// CHUNK_SIZE. The client of `ChunkProvider` has exclusive access to this
    /// slice thereafter.
    fn allocate(&mut self, num_chunks: usize) -> *mut [MaybeUninit<u8>];
}

pub struct Heap<
    Provider,
    const CHUNK_SIZE: usize = { crate::memory::page::PAGE_SIZE.as_raw() as usize },
> {
    free_lists: [sll::SinglyLinkedList<BlockAdapter>; NUM_BLOCK_SIZES],
    provider: Provider,
}

#[derive(Clone, Default)]
struct BlockAdapter {
    link_ops: sll::AtomicLinkOps,
    pointer_ops: intrusive_collections::DefaultPointerOps<UnsafeRef<FreeBlock>>,
}

unsafe impl Adapter for BlockAdapter {
    type LinkOps = sll::AtomicLinkOps;
    type PointerOps = intrusive_collections::DefaultPointerOps<UnsafeRef<FreeBlock>>;

    unsafe fn get_link(
        &self,
        value: *const <Self::PointerOps as intrusive_collections::PointerOps>::Value,
    ) -> <Self::LinkOps as intrusive_collections::LinkOps>::LinkPtr {
        unsafe { NonNull::new_unchecked(addr_of!((*value).header.link) as *mut _) }
    }

    unsafe fn get_value(
        &self,
        link: <Self::LinkOps as intrusive_collections::LinkOps>::LinkPtr,
    ) -> *const <Self::PointerOps as intrusive_collections::PointerOps>::Value {
        let offset = memoffset::offset_of!(FreeBlockData, link);
        // SAFETY: `link` points to the `link` field of a `FreeBlockData`. We
        // offset the pointer accordingly and get a reference to the owning
        // `FreeBlockData`.
        let header =
            unsafe { &*(link.as_ptr().byte_offset(-(offset as isize)) as *const FreeBlockData) };

        let size = header.size.size();
        core::ptr::from_raw_parts::<FreeBlock>(
            header as *const _ as *const (),
            FreeBlock::metadata_from_size(size),
        )
    }

    fn link_ops(&self) -> &Self::LinkOps {
        &self.link_ops
    }

    fn link_ops_mut(&mut self) -> &mut Self::LinkOps {
        &mut self.link_ops
    }

    fn pointer_ops(&self) -> &Self::PointerOps {
        &self.pointer_ops
    }
}

// intrusive_adapter!(BlockAdapter = UnsafeRef<FreeBlock>: FreeBlock { header: FreeBlockData { link: sll::AtomicLink }});

impl<Provider: ChunkProvider<CHUNK_SIZE>, const CHUNK_SIZE: usize> Heap<Provider, CHUNK_SIZE> {
    pub fn new(provider: Provider) -> Self {
        // Ideally this would be a static assertion like in C++, but I can't
        // figure out how. This will almost definitely be optimized out anyway.
        assert!(CHUNK_SIZE >= *BLOCK_SIZES.last().unwrap());
        assert!(CHUNK_SIZE.is_power_of_two());
        Heap {
            free_lists: core::array::from_fn(|_| {
                sll::SinglyLinkedList::new(BlockAdapter::default())
            }),
            provider,
        }
    }

    fn fetch_chunk(&mut self) {
        let chunk_ptr = self.provider.allocate(1);

        // For little runtime cost, double-check `provider` met its
        // requirements.
        assert_eq!(chunk_ptr.len(), CHUNK_SIZE);
        assert!(chunk_ptr.is_aligned_to(CHUNK_SIZE));

        // These checks are a little paranoid but they can't hurt. They only
        // depend on const-evaluable expressions so hopefully they'll be
        // optimized out.
        assert!(chunk_ptr.cast::<FreeBlockData>().is_aligned());
        assert!(chunk_ptr.len() >= core::mem::size_of::<FreeBlockData>());

        // SAFETY: `provider` is guaranteed to return a valid pointer to
        // CHUNK_SIZE size and aligned bytes, and `MaybeUninit<_>` means we can
        // create a reference despite it not being initialized.
        let mut chunk: &'static mut [MaybeUninit<u8>] = unsafe { &mut *chunk_ptr };

        let free_list = self.free_lists.last_mut().unwrap();
        while chunk.len() >= MAXIMAL_BLOCK_SIZE {
            let block;
            (block, chunk) = FreeBlock::build(chunk, BlockSizeKey::Size256);
            free_list.push_front(unsafe { UnsafeRef::from_raw(block as *mut _) });
        }
    }

    /// Number of maximally-sized blocks we can fit in a chunk. Hopefully it
    /// divides evenly.
    const BLOCKS_PER_CHUNK: usize = CHUNK_SIZE / MAXIMAL_BLOCK_SIZE;
}

const NUM_BLOCK_SIZES: usize = 5;
const BLOCK_SIZES: [usize; NUM_BLOCK_SIZES] = [16, 32, 64, 128, 256];
const MAXIMAL_BLOCK_SIZE: usize = *BLOCK_SIZES.last().unwrap();

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
#[repr(usize)]
enum BlockSizeKey {
    Size16 = 0,
    Size32 = 1,
    Size64 = 2,
    Size128 = 3,
    Size256 = 4,
}

impl BlockSizeKey {
    const fn size(self) -> usize {
        BLOCK_SIZES[self as usize]
    }
}

#[repr(C)]
#[derive(Debug)]
struct FreeBlockData {
    size: BlockSizeKey,
    link: sll::AtomicLink,
}

const_assert!(core::mem::size_of::<FreeBlockData>() <= BLOCK_SIZES[0]);

#[repr(C)]
#[derive(Debug)]
struct FreeBlock {
    header: FreeBlockData,
    _rest: [MaybeUninit<u8>],
}

impl FreeBlock {
    /// Construct `FreeBlock` from a reference to a block. This constructs the
    /// block header and returns a `&mut FreeBlock`, which is dynamically-sized.
    /// It also returns a reference to the remaining memory after the block.
    fn build<'a>(
        mem: &'a mut [MaybeUninit<u8>],
        size: BlockSizeKey,
    ) -> (&'a mut FreeBlock, &'a mut [MaybeUninit<u8>]) {
        let (block_mem, rest) = mem.split_at_mut(size.size());
        let block_len = block_mem.len();

        // Get a pointer to write our header at the start of the block.
        let block_header = block_mem as *mut _ as *mut FreeBlockData;
        assert!(block_header.is_aligned());

        // Write the block header.
        //
        // SAFETY: `block_header` is valid and aligned. Since it's derived from an
        // exclusive reference, it is safe to write to it.
        unsafe {
            block_header.write(FreeBlockData {
                size,
                link: sll::AtomicLink::new(),
            });
        }

        // Create a `FreeBlock` pointer, where the written `FreeBlockData` value
        // coincides with `FreeBlock`'s `header` field. Note that `block`'s
        // metadata is the size of the `_rest` field, not of the entire value.
        let block: *mut FreeBlock = core::ptr::from_raw_parts_mut(
            block_mem as *mut _ as *mut (),
            Self::metadata_from_size(block_len),
        );

        // SAFETY: `block` points to a valid `FreeBlockData` value followed by
        // uninitialized data, which matches the representation of `FreeBlock`.
        // `block` is correctly sized for the `_rest` field, and it is properly
        // aligned.
        assert_eq!(block_len, core::mem::size_of_val(unsafe { &*block }));
        let block: &mut FreeBlock = unsafe { &mut *block };

        (block, rest)
    }

    fn metadata_from_size(size: usize) -> usize {
        size - core::mem::size_of::<FreeBlockData>()
    }
}

#[cfg(test)]
mod test {
    use super::*;

    use test_log::test;

    const PAGE_SIZE: usize = crate::memory::page::PAGE_SIZE.as_raw() as usize;

    #[test]
    fn block_build() {
        let mut mem_array = aligned::Aligned::<aligned::A64, _>([MaybeUninit::uninit(); PAGE_SIZE]);
        let mem = &mut *mem_array;
        let (block, rest) = FreeBlock::build(mem, BlockSizeKey::Size256);

        assert_eq!(block.header.size, BlockSizeKey::Size256);
        assert_eq!(core::mem::size_of_val(&*block), 256);
    }

    // Unfortunately, the intrusive list techniques violate stacked borrow
    // rules.
    #[cfg(not(miri))]
    #[test]
    fn heap() {
        let mut heap = Heap::new(TestProvider {
            allocations: Vec::new(),
        });
        heap.fetch_chunk();

        let free_list = heap.free_lists.last_mut().unwrap();
        for block in free_list.iter() {
            assert_eq!(core::mem::size_of_val(block), block.header.size.size());
            assert_eq!(BlockSizeKey::Size256, block.header.size);
        }

        while let Some(block) = free_list.pop_front() {
            let block = unsafe { &*UnsafeRef::into_raw(block) };
            assert_eq!(core::mem::size_of_val(block), block.header.size.size());
            assert_eq!(BlockSizeKey::Size256, block.header.size);
        }
    }

    struct TestProvider {
        /// To avoid memory leaks in tests, keep track of pointers and dealloc
        /// them later. In the kernel this doesn't matter; the heap lives
        /// forever.
        allocations: Vec<(*mut u8, std::alloc::Layout)>,
    }

    impl Drop for TestProvider {
        fn drop(&mut self) {
            for (p, l) in self.allocations.drain(..) {
                unsafe {
                    std::alloc::dealloc(p, l);
                }
            }
        }
    }

    unsafe impl ChunkProvider for TestProvider {
        fn allocate(&mut self, num_chunks: usize) -> *mut [MaybeUninit<u8>] {
            use std::alloc::*;

            let len = num_chunks * PAGE_SIZE;
            let layout = Layout::from_size_align(len, PAGE_SIZE).unwrap();
            let raw = unsafe { alloc(layout) };
            assert!(!raw.is_null());
            self.allocations.push((raw, layout));

            core::ptr::slice_from_raw_parts_mut(raw as *mut MaybeUninit<u8>, len)
        }
    }
}
