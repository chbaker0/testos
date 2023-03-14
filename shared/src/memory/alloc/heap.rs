//! A simple heap allocator for arbitrary-sized allocations.

use core::alloc::{AllocError, Allocator, GlobalAlloc, Layout};
use core::mem::MaybeUninit;
use core::ptr::{addr_of, NonNull};

use intrusive_collections::UnsafeRef;
use intrusive_collections::{singly_linked_list as sll, Adapter};
use num_traits::{FromPrimitive, ToPrimitive};
use spin::Mutex;
use static_assertions::const_assert;

pub const DEFAULT_CHUNK_SIZE: usize = crate::memory::page::PAGE_SIZE.as_raw() as usize;

/// Provides backing memory to `Heap`. `CHUNK_SIZE` must be a power of 2.
///
/// # Safety
///
/// Follow safety comments on methods.
pub unsafe trait ChunkProvider<const CHUNK_SIZE: usize = DEFAULT_CHUNK_SIZE> {
    /// Allocate `num_chunks` contiguous chunks.
    ///
    /// # Safety
    ///
    /// The implementation must return a valid slice sized and aligned to
    /// CHUNK_SIZE. The client of `ChunkProvider` has exclusive access to this
    /// slice thereafter.
    fn allocate(&mut self, num_chunks: usize) -> *mut [MaybeUninit<u8>];
}

pub struct Heap<Provider, const CHUNK_SIZE: usize = DEFAULT_CHUNK_SIZE> {
    free_lists: [sll::SinglyLinkedList<BlockAdapter>; NUM_BLOCK_SIZES],
    provider: Provider,
}

#[derive(Clone, Default)]
struct BlockAdapter {
    link_ops: sll::AtomicLinkOps,
    pointer_ops: intrusive_collections::DefaultPointerOps<UnsafeRef<FreeBlock>>,
}

impl BlockAdapter {
    const fn new() -> Self {
        BlockAdapter {
            link_ops: sll::AtomicLinkOps,
            pointer_ops: intrusive_collections::DefaultPointerOps::new(),
        }
    }
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

impl<Provider: ChunkProvider<CHUNK_SIZE>, const CHUNK_SIZE: usize> Heap<Provider, CHUNK_SIZE> {
    pub const fn new(provider: Provider) -> Self {
        // Ideally this would be a static assertion like in C++, but I can't
        // figure out how. This will almost definitely be optimized out anyway.
        assert!(CHUNK_SIZE >= *BLOCK_SIZES.last().unwrap());
        assert!(CHUNK_SIZE.is_power_of_two());
        Heap {
            free_lists: [
                sll::SinglyLinkedList::new(BlockAdapter::new()),
                sll::SinglyLinkedList::new(BlockAdapter::new()),
                sll::SinglyLinkedList::new(BlockAdapter::new()),
                sll::SinglyLinkedList::new(BlockAdapter::new()),
                sll::SinglyLinkedList::new(BlockAdapter::new()),
            ],
            provider,
        }
    }

    fn allocate(&mut self, layout: Layout) -> *mut [u8] {
        let key = match self.key_for_size_align(layout.size(), layout.align()) {
            Some(key) => key,
            None => {
                let chunks = layout.size().div_ceil(CHUNK_SIZE);
                let ptr: *mut [MaybeUninit<u8>] = self.provider.allocate(chunks);
                return ptr as *mut [u8];
            }
        };

        self.allocate_small(key, layout)
    }

    fn allocate_small(&mut self, key: BlockSizeKey, layout: Layout) -> *mut [u8] {
        let first_fit: &mut sll::SinglyLinkedList<_> = match self.free_lists
            [key.to_usize().unwrap()..]
            .iter_mut()
            .find(|l| !l.is_empty())
        {
            Some(l) => l,
            None => {
                self.fetch_chunk();
                return self.allocate_small(key, layout);
            }
        };

        let block_ptr = UnsafeRef::into_raw(first_fit.pop_front().unwrap());
        assert!(block_ptr.is_aligned_to(layout.align()));
        let block = unsafe { &mut *block_ptr };
        assert!(block.header.size.size() >= layout.size());

        // The data in `block` does not need to be dropped. It was already
        // unlinked from the list. It can be returned directly as a pointer,
        // taking into account the size.
        core::ptr::slice_from_raw_parts_mut(block_ptr as *mut u8, layout.size())
    }

    /// Get the smallest `BlockSizeKey` to fit `size`, or `None` if no block
    /// size is large enough.
    fn key_for_size_align(&mut self, size: usize, align: usize) -> Option<BlockSizeKey> {
        let size = core::cmp::max(size, align);
        let key_ndx = match BLOCK_SIZES.binary_search(&size) {
            Ok(ndx) => ndx,
            // Too big...need to allocate chunks directly for this.
            Err(NUM_BLOCK_SIZES) => return None,
            // `ndx` is the insertion point for `size` to keep it sorted. This
            // means it points to the first element larger than `size`, which
            // is what we want.
            Err(ndx) => ndx,
        };

        Some(BlockSizeKey::from_usize(key_ndx).unwrap())
    }

    /// Get a new chunk from the system and link in its free blocks.
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
}

const NUM_BLOCK_SIZES: usize = 5;
const BLOCK_SIZES: [usize; NUM_BLOCK_SIZES] = [16, 32, 64, 128, 256];
const MAXIMAL_BLOCK_SIZE: usize = *BLOCK_SIZES.last().unwrap();

pub struct CheckedHeap<Provider, const CHUNK_SIZE: usize = DEFAULT_CHUNK_SIZE>(
    pub Mutex<Heap<Provider, CHUNK_SIZE>>,
);

impl<Provider, const CHUNK_SIZE: usize> CheckedHeap<Provider, CHUNK_SIZE> {
    pub const fn new(heap: Heap<Provider, CHUNK_SIZE>) -> Self {
        CheckedHeap(Mutex::new(heap))
    }

    pub fn get(&self) -> spin::MutexGuard<Heap<Provider, CHUNK_SIZE>> {
        self.0.try_lock().unwrap()
    }
}

unsafe impl<Provider: ChunkProvider<CHUNK_SIZE>, const CHUNK_SIZE: usize> GlobalAlloc
    for CheckedHeap<Provider, CHUNK_SIZE>
{
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        self.get().allocate(layout) as *mut u8
    }

    unsafe fn dealloc(&self, _ptr: *mut u8, _layout: Layout) {
        // Do nothing for now...
    }
}

unsafe impl<Provider: ChunkProvider<CHUNK_SIZE>, const CHUNK_SIZE: usize> Allocator
    for CheckedHeap<Provider, CHUNK_SIZE>
{
    fn allocate(&self, layout: Layout) -> Result<NonNull<[u8]>, core::alloc::AllocError> {
        NonNull::new(self.0.try_lock().ok_or(AllocError)?.allocate(layout)).ok_or(AllocError)
    }

    unsafe fn deallocate(&self, _ptr: NonNull<u8>, _layout: Layout) {
        // Do nothing for now...
    }
}

#[derive(
    Clone,
    Copy,
    Debug,
    Eq,
    Ord,
    PartialEq,
    PartialOrd,
    num_derive::FromPrimitive,
    num_derive::ToPrimitive,
)]
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
    fn build(
        mem: &mut [MaybeUninit<u8>],
        size: BlockSizeKey,
    ) -> (&mut FreeBlock, &mut [MaybeUninit<u8>]) {
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

    #[test]
    fn heap() {
        let mut heap = Heap::new(TestProvider {
            allocations: Vec::new(),
        });

        // Fetch a bunch of chunks and see what happens.
        for i in 0..50 {
            heap.fetch_chunk();
        }

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

    // Using standard collections with `Heap` should be enough of a stress test.
    #[test]
    fn test_heap_with_collections() {
        let provider = TestProvider {
            allocations: Vec::new(),
        };
        let allocator = CheckedHeap(Mutex::new(Heap::new(provider)));
        let mut vec = Vec::new_in(&allocator);
        for i in 0..1000 {
            vec.push(i);
        }

        let mut set = std::collections::HashSet::new();
        for i in 0..1000 {
            set.insert(i);
        }

        for i in (0..1000).rev() {
            set.remove(&i);
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
