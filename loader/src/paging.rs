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
            Some(self.0.entries[ndx].addr())
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

pub struct AddrSpace {
    p4: *mut Table<Level4>,
}

impl AddrSpace {
    pub fn new(alloc: &mut FrameAllocator) -> AddrSpace {
        let tablep = alloc.get_frame() as *mut Table<Level4>;
        let table = unsafe { &mut *tablep };
        table.0.zero();
        table.0.entries[510].0 = (tablep as u64) | 0b1001;
        AddrSpace {
            p4: tablep,
        }
    }

    pub fn map_to(&mut self, page: Page, frame: Frame, flags: u64, alloc: &mut FrameAllocator) {
        let p4 = unsafe { &mut *self.p4 };
        let p3 = p4.next_create(page.p4_ndx(), alloc);
        let p2 = p3.next_create(page.p3_ndx(), alloc);
        let p1 = p2.next_create(page.p2_ndx(), alloc);
        let entry = &mut p1.0.entries[page.p1_ndx()].0;
        *entry = (frame.0 << 12) | flags | 1;
    }

    pub fn get_p4_addr(&self) -> u64 {
        self.p4 as u64
    }
}
