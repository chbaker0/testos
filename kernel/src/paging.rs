use shared;
use shared::memory::FrameAllocator;

pub use shared::paging::Frame;
pub use shared::paging::Page;
use shared::paging::Entry;
use shared::paging::TableLevel;
use shared::paging::Level4;
use shared::paging::Level3;
use shared::paging::Level2;
use shared::paging::Level1;
use shared::paging::HierarchicalLevel;

#[repr(C, packed)]
struct Table<L: TableLevel>(pub shared::paging::Table<L>);

impl<L: HierarchicalLevel> Table<L>{
    fn next_addr(&self, ndx: usize) -> Option<u64> {
        if self.0.entries[ndx].flags() & 1 == 0 {
            None
        } else {
            let cur_addr = self as *const _ as u64;
            Some((0xffff << 48) | (cur_addr << 9) | ((ndx as u64) << 12))
        }
    }

    pub fn next(&self, ndx: usize) -> Option<&Table<L::NextLevel>> {
        self.next_addr(ndx).map(|addr| unsafe { &*(addr as *const _) })
    }

    pub fn next_mut(&mut self, ndx: usize) -> Option<&mut Table<L::NextLevel>> {
        self.next_addr(ndx).map(|addr| unsafe { &mut *(addr as *mut _) })
    }

    pub fn next_create(&mut self, ndx: usize, alloc: &mut FrameAllocator)
                                              -> &mut Table<L::NextLevel> {
        if self.next(ndx).is_none() {
            let frame_addr = alloc.get_frame() as u64;
            self.0.entries[ndx].0 = frame_addr | 0b1001; // Set writable and present bits.
            self.next_mut(ndx).unwrap().0.zero();
        }
        self.next_mut(ndx).unwrap()
    }
}

// We assume paging is already set up and that the second-to-last
// entry of P4 is mapped to itself.
const P4: *mut Table<Level4> = 0o177777_776_776_776_776_0000 as *mut _;

pub fn map_to(page: Page, frame: Frame, flags: u64, alloc: &mut FrameAllocator) {
    let p4 = unsafe { &mut *P4 };
    let p3 = p4.next_create(page.p4_ndx(), alloc);
    let p2 = p3.next_create(page.p3_ndx(), alloc);
    let p1 = p2.next_create(page.p2_ndx(), alloc);
    let entry = &mut p1.0.entries[page.p1_ndx()].0;
    *entry = (frame.0 << 12) | flags | 1;
}
