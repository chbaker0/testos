use alloc::boxed::Box;
use core::cell::Cell;
use intrusive_collections::{LinkedList, LinkedListLink};
use intrusive_collections::linked_list;

pub struct AddressSpace {
    free_list: LinkedList<RegionAdapter>,
}

pub struct Region {
    link: LinkedListLink,
    pub first_addr: Cell<u64>,
    pub last_addr: Cell<u64>,
}

intrusive_adapter!(pub RegionAdapter = Box<Region>: Region { link: LinkedListLink });

impl AddressSpace {
    fn create_node(first_addr: u64, last_addr: u64) -> Box<Region> {
        Box::new(Region {
            link: LinkedListLink::new(),
            first_addr: Cell::new(first_addr),
            last_addr: Cell::new(last_addr),
        })
    }

    pub fn new(first_addr: u64, last_addr: u64) -> AddressSpace {
        let node = AddressSpace::create_node(first_addr, last_addr);
        let mut free_list = LinkedList::new(RegionAdapter::new());
        free_list.push_back(node);
        AddressSpace {
            free_list: free_list,
        }
    }

    pub fn reserve(&mut self, first: u64, last: u64) {
        let mut cur = self.free_list.cursor_mut();
        cur.move_next();
        while !cur.is_null() {
            let ref region = cur.get().unwrap();
            if first > region.first_addr.get() && last < region.last_addr.get() {
                // Input range is completely contained in current region.
                let old_last = region.last_addr.replace(first-1);
                cur.insert_after(Self::create_node(last+1, old_last));
                break;
            } else if first > region.first_addr.get() && last == region.last_addr.get() {
                // Input range ends where current region ends.
                region.last_addr.set(first-1);
                break;
            } else if first == region.first_addr.get() && last < region.last_addr.get() {
                // Input range starts where current region starts.
                region.first_addr.set(last+1);
                break;
            } else if first == region.first_addr.get() && last == region.last_addr.get() {
                // Input range equals current region.
                cur.remove();
                break;
            } else if first > region.last_addr.get() {
                cur.move_next();
            } else {
                panic!("Attempted to reserve unmanaged virtual memory.");
            }
        }
    }

    pub fn unreserve(&mut self, first: u64, last: u64) {
        let mut cur = self.free_list.cursor_mut();
        cur.move_next();
        // Insert or merge with existing region.
        while !cur.is_null() {
            let ref region = cur.get().unwrap();
            if first == region.last_addr.get() + 1 {
                // Extend existing region.
                region.last_addr.set(last);
                break;
            } else if first > region.last_addr.get() + 1 {
                // We found a spot.
                cur.insert_after(Self::create_node(first, last));
                cur.move_next();
                break;
            } else if first <= region.last_addr.get() && last >= region.last_addr.get() {
                panic!("Attempted to unreserve virtual memory that is already free.");
            }
        }

        // Try to merge with next region if we can.
        let mut remove_next = false;
        {
            let next = cur.peek_next();
            if !next.is_null() {
                let ref region = next.get().unwrap();
                if region.first_addr.get() == last+1 {
                    cur.get().unwrap().last_addr.set(region.last_addr.get());
                    remove_next = true;
                } else if region.first_addr.get() <= last {
                    panic!("Attempted to unreserve virtual memory that is already free.");
                }
            }
        }

        if remove_next {
            cur.move_next();
            cur.remove();
        }
    }

    pub fn allocate(&mut self, size: u64) -> Result<u64, ()> {
        assert!(size > 0);
        let mut cur = self.free_list.cursor_mut();
        cur.move_next();
        while !cur.is_null() {
            let ref region = cur.get().unwrap();
            if region.last_addr.get() - region.first_addr.get() + 1 > size {
                let old = region.first_addr.get();
                region.first_addr.set(old + size);
                return Ok(old);
            } else if region.last_addr.get() - region.first_addr.get() + 1 == size {
                let old = region.first_addr.get();
                cur.remove();
                return Ok(old);
            }
            cur.move_next();
        }

        Err(())
    }

    pub fn iter(&self) -> linked_list::Iter<RegionAdapter> {
        self.free_list.iter()
    }
}
