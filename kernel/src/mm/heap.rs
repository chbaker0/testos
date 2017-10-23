use super::FrameAllocator;
use super::paging;
use super::physmem;

use core::mem;
use core::ptr::null_mut;
use spin;

pub const HEAP_START_ADDR: usize = 0xffff_fe80_0000_0000;
pub const HEAP_END_ADDR: usize = 0xffff_ff00_0000_0000;

struct Heap {
    start_addr: usize,
    end_addr: usize,
    cur_break: usize,
    list_head: *mut BlockHeader,
}

unsafe impl Send for Heap {}

fn align_up(num: usize, align: usize) -> usize {
    assert!(align == 0 || align.is_power_of_two());
    if align == 0 {
        num
    } else {
        (num + align - 1) & !(align - 1)
    }
}

#[repr(packed)]
struct BlockHeader {
    size: usize,
    next: *mut BlockHeader,
}

unsafe impl Send for BlockHeader {}

impl Heap {
    const fn new(start_addr: usize, end_addr: usize) -> Heap {
        Heap {
            start_addr: start_addr,
            end_addr: end_addr,
            cur_break: start_addr,
            list_head: null_mut(),
        }
    }

    fn fits(alloc_size: usize, alloc_align: usize, block_addr: usize, block_size: usize) -> bool {
        let aligned_addr = align_up(block_addr, alloc_align);
        let aligned_size = block_size - (aligned_addr - block_addr);
        alloc_size <= aligned_size
    }

    fn insert_block_at(&mut self, addr: usize, size: usize, prev: *mut BlockHeader) {
        assert!(size >= mem::size_of::<BlockHeader>());
        let block_header = addr as *mut BlockHeader;
        let next_block =
            if prev == null_mut() {
                null_mut()
            } else {
                unsafe { (*prev).next }
            };

        unsafe { *block_header = BlockHeader {size: size, next: next_block} };
        if prev == null_mut() {
            self.list_head = block_header
        } else {
            unsafe { (*prev).next = block_header };
        }
    }

    fn remove_block(&mut self, block: *mut BlockHeader, prev: *mut BlockHeader) {
        assert!(block != null_mut());
        let next = unsafe { (*block).next };
        if prev == null_mut() {
            self.list_head = next;
        } else {
            unsafe { (*prev).next = next};
        }
    }

    fn allocate_raw(&mut self, size: usize, align: usize, alloc: &mut FrameAllocator) -> *mut u8 {
        assert!(align <= paging::PAGE_SIZE);
        let header_size = mem::size_of::<BlockHeader>().next_power_of_two();
        let aligned_size = align_up(size, header_size);

        // Try to find a block that fits the allocation.
        let mut prev_block = null_mut();
        let mut block = self.list_head;
        while block != null_mut() {
            if Heap::fits(aligned_size, align, block as usize, unsafe { (*block).size }) {
                break;
            } else {
                prev_block = block;
                block = unsafe { (*block).next };
            }
        }

        if block != null_mut() {
            // We found a block, use it.
            let addr = block as usize;
            let block_size = unsafe { (*block).size };
            assert!(block_size >= aligned_size);

            self.remove_block(block, prev_block);

            let space_left = block_size - aligned_size;
            if space_left >= header_size {
                self.insert_block_at(addr + aligned_size, space_left, prev_block);
            }

            addr as *mut u8

        } else {
            // No block found, move break and possibly add a new free block at end.
            let addr = self.cur_break;
            let pages = (aligned_size + paging::PAGE_SIZE - 1) / paging::PAGE_SIZE;
            for _ in 0..pages {
                self.add_page(alloc);
            }

            let space_left = pages * paging::PAGE_SIZE - aligned_size;
            if space_left >= header_size {
                self.insert_block_at(addr + aligned_size, space_left, prev_block);
            }

            addr as *mut u8
        }
    }

    fn try_merge_with_next(&mut self, block: *mut BlockHeader) {
        let size = unsafe { (*block).size };
        let next = unsafe { (*block).next };
        if next != null_mut() && (block as usize) + size == (next as usize) {
            unsafe {
                (*block).size += (*next).size;
                (*block).next = (*next).next;
            }
        }
    }

    fn deallocate(&mut self, ptr: *mut u8, size: usize, align: usize) {
        let header_size = mem::size_of::<BlockHeader>().next_power_of_two();
        let aligned_size = align_up(size, header_size);

        // Find blocks before and after newly freed block.
        let mut next_block = self.list_head;
        let mut prev_block = null_mut();
        while next_block != null_mut() && (next_block as usize) < (ptr as usize) {
            prev_block = next_block;
            next_block = unsafe { (*next_block).next };
        }

        // Insert new block into linked list.
        let block = ptr as *mut BlockHeader;
        unsafe { (*block) = BlockHeader{size: aligned_size, next: next_block}; }
        if prev_block == null_mut() {
            self.list_head = block;
        } else {
            unsafe { (*prev_block).next = block; }
        }

        // Merge free blocks
        self.try_merge_with_next(block);
        if prev_block != null_mut() {
            self.try_merge_with_next(prev_block);
        }
    }

    fn allocate<T>(&mut self, alloc: &mut FrameAllocator) -> *mut T {
        self.allocate_raw(mem::size_of::<T>(), mem::align_of::<T>(), alloc) as *mut T
    }

    fn add_page(&mut self, alloc: &mut FrameAllocator) {
        let addr = alloc.get_frame();
        paging::map_to(paging::Page((self.cur_break / paging::PAGE_SIZE) as u64),
                       paging::Frame((addr / paging::PAGE_SIZE) as u64),
                       0b1010, alloc);
        self.cur_break += paging::PAGE_SIZE;
    }
}

lazy_static! {
    static ref HEAP: spin::Mutex<Heap> = {
        spin::Mutex::new(Heap::new(HEAP_START_ADDR, HEAP_END_ADDR))
    };
}

pub fn allocate_raw(size: usize, align: usize) -> *mut u8 {
    HEAP.lock().allocate_raw(size, align, physmem::get_frame_allocator())
}

pub fn allocate<T>() -> *mut T {
    HEAP.lock().allocate(physmem::get_frame_allocator())
}

pub fn init() {
    // Do nothing for now.
}
