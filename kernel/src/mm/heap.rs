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
            let next_block = unsafe { (*block).next };
            assert!(block_size >= aligned_size);

            let space_left = block_size - aligned_size;
            if space_left >= header_size {
                let new_block = (addr + aligned_size) as *mut BlockHeader;
                unsafe {
                    *new_block = BlockHeader {size: space_left, next: next_block};
                    if prev_block != null_mut() {
                        (*prev_block).next = new_block;
                    } else {
                        self.list_head = new_block;
                    }
                }
            } else {
                unsafe {
                    if prev_block != null_mut() {
                        (*prev_block).next = next_block;
                    } else {
                        self.list_head = next_block;
                    }
                }
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
                let new_block = (addr + aligned_size) as *mut BlockHeader;
                unsafe {
                    *new_block = BlockHeader {size: space_left, next: null_mut()};
                    if prev_block != null_mut() {
                        (*prev_block).next = new_block;
                    } else {
                        self.list_head = new_block;
                    }
                }
            }

            addr as *mut u8
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
