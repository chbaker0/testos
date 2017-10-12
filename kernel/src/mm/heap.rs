use super::FrameAllocator;
use super::paging;

use core::mem;

pub const HEAP_START_ADDR: usize = 0xffff_fe80_0000_0000;
pub const HEAP_END_ADDR: usize = 0xffff_ff00_0000_0000;

struct Heap {
    start_addr: usize,
    end_addr: usize,
    cur_break: usize,
    cur_addr: usize,
}

impl Heap {
    const fn new(start_addr: usize, end_addr: usize) -> Heap {
        Heap {
            start_addr: start_addr,
            end_addr: end_addr,
            cur_break: start_addr,
            cur_addr: start_addr,
        }
    }

    fn allocate_raw(&mut self, size: usize, align: usize, alloc: &mut FrameAllocator) -> *mut u8 {
        assert!(align == 0 || align.is_power_of_two());
        let addr =
            if align > 0 {
                (self.cur_addr + align - 1) & !(align - 1)
            } else {
                self.cur_addr
            };

        self.cur_addr = addr + size;
        while self.cur_addr < self.cur_break {
            self.add_page(alloc);
        }

        addr as *mut u8
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

pub fn init() {
}
