use super::FrameAllocator;
use super::paging;

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

    fn add_page(&mut self, alloc: &mut FrameAllocator) {
        let addr = alloc.get_frame();
        paging::map_to(paging::Page((self.cur_break / paging::PAGE_SIZE) as u64),
                       paging::Frame((addr / paging::PAGE_SIZE) as u64),
                       0b1010, alloc);
        self.cur_break += paging::PAGE_SIZE;
    }
}

static mut HEAP: Heap = Heap::new(HEAP_START_ADDR, HEAP_END_ADDR);

pub fn init() {
}
