/* Page table management
 *
 * This implementation is largely taken from
 * https://os.phil-opp.com/page-tables/ and is licensed under the
 * MIT or Apache 2.0 license. See https://github.com/phil-opp/blog_os/
 * for full source.
 */

use core::marker::PhantomData;

const ENTRY_COUNT: usize = 512;

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
#[derive(Clone, Copy)]
pub struct Entry(pub u64);

impl Entry {
    pub fn flags(&self) -> u64 {
        self.0 & 0xfff00000_00000fff
    }

    pub fn addr(&self) -> u64 {
        self.0 & 0x000fffff_fffff000
    }
}

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
    pub entries: [Entry; ENTRY_COUNT],
    pub level: PhantomData<L>,
}

impl<L: TableLevel> Table<L> {
    pub fn zero(&mut self) {
        for e in self.entries.iter_mut() {
            e.0 = 0;
        }
    }
}
