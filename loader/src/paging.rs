/* Page table management
 *
 * This implementation is largely taken from
 * https://os.phil-opp.com/page-tables/ and is licensed under the
 * MIT or Apache 2.0 license. See https://github.com/phil-opp/blog_os/
 * for full source.
 */

use shared::memory::FrameAllocator;
use shared::memory::PAGE_SIZE;

use core::marker::PhantomData;
use core::option::Option;

const ENTRY_BITS: usize = 9;
const ENTRY_COUNT: usize = 512;
const VADDR_BITS: usize = 48;

pub struct Frame(pub u64);
pub struct Page(pub u64);

impl Page {
    pub fn p4_ndx(&self) -> usize {
        ((self.0 >> 27) & 0o777) as usize
    }

    pub fn p3_ndx(&self) -> usize {
        ((self.0 >> 18) & 0o777) as usize
    }

    pub fn p2_ndx(&self) -> usize {
        ((self.0 >> 9) & 0o777) as usize
    }

    pub fn p1_ndx(&self) -> usize {
        ((self.0 >> 0) & 0o777) as usize
    }
}

#[repr(C, packed)]
pub struct Entry(u64);

pub trait TableLevel {}

pub enum Level4 {}
pub enum Level3 {}
pub enum Level2 {}
pub enum Level1 {}

impl TableLevel for Level4 {}
impl TableLevel for Level3 {}
impl TableLevel for Level2 {}
impl TableLevel for Level1 {}

pub trait HierarchicalLevel: TableLevel {
    type NextLevel: TableLevel;
}

impl HierarchicalLevel for Level4 {
    type NextLevel = Level3;
}

impl HierarchicalLevel for Level3 {
    type NextLevel = Level2;
}

impl HierarchicalLevel for Level2 {
    type NextLevel = Level1;
}

#[repr(C, packed)]
pub struct Table<L: TableLevel> {
    entries: [Entry; ENTRY_COUNT],
    level: PhantomData<L>,
}

impl<L: TableLevel> Table<L> {
    pub fn zero(&mut self) {
        for e in self.entries.iter_mut() {
            e.0 = 0;
        }
    }
}

impl<L: HierarchicalLevel> Table<L>{
    fn next_addr(&self, ndx: usize) -> Option<u64> {
        let Entry(addr) = self.entries[ndx];
        if addr & 1 == 0 {
            None
        } else {
            Some(addr)
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
            self.entries[ndx].0 = frame_addr | 0b1001; // Set writable and present bits.
            self.next_mut(ndx).unwrap().zero();
        }
        self.next_mut(ndx).unwrap()
    }
}

pub struct AddrSpace {
    p4: Table<Level4>,
}

impl AddrSpace {
    pub fn check_vaddr(addr: u64) -> bool {
        let sign = (addr >> VADDR_BITS) & 1;
        let high_bits = addr >> VADDR_BITS;
        high_bits == (sign << (64 - VADDR_BITS)) - 1
    }

    pub fn map_to(&mut self, page: Page, frame: Frame, flags: u64, alloc: &mut FrameAllocator) {
        let p3 = self.p4.next_create(page.p4_ndx(), alloc);
        let p2 = p3.next_create(page.p3_ndx(), alloc);
        let p1 = p2.next_create(page.p2_ndx(), alloc);
        let entry = &mut p1.entries[page.p1_ndx()].0;
        *entry = (frame.0 << 12) | flags | 1;
    }
}
