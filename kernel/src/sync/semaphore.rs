use intrusive_collections::{LinkedList, LinkedListLink, UnsafeRef};
use spin;

use ::sched;

struct WaitListNode {
    thread: u64,
    link: LinkedListLink
}

intrusive_adapter!(WaitListAdapter = UnsafeRef<WaitListNode>: WaitListNode { link: LinkedListLink });

struct SemaphoreInternal {
    value: i64,
    wait_list: LinkedList<WaitListAdapter>
}

pub struct Semaphore {
    lock: spin::Mutex<SemaphoreInternal>,
}

unsafe impl Send for Semaphore {

}

unsafe impl Sync for Semaphore {

}

impl Semaphore {
    pub fn new(value: i64) -> Semaphore {
        Semaphore {
            lock: spin::Mutex::new(SemaphoreInternal {
                value: value,
                wait_list: LinkedList::new(WaitListAdapter::new()),
            }),
        }
    }

    pub fn wait(&self) {
        let wait_list_node = WaitListNode {
            thread: sched::cur_thread(),
            link: LinkedListLink::new(),
        };

        sched::block_cur();

        {
            let mut lock = self.lock.lock();
            lock.value -= 1;

            if lock.value < 0 {
                lock.wait_list.push_back(unsafe { UnsafeRef::from_raw(&wait_list_node as *const _) });
            } else {
                sched::unblock_cur();
                return;
            }
        }

        // If we reach this point we must block.
        sched::yield_cur();

        // By this point, another thread has signalled, removed us
        // from the wait list, and woken us up. We can now return.
    }

    pub fn signal(&self) {
        let mut lock = self.lock.lock();
        lock.value += 1;
        // Wake up a thread if one is waiting.
        if lock.value <= 0 {
            assert!(!lock.wait_list.is_empty());
            let noderef = lock.wait_list.pop_front().unwrap();
            let thread = unsafe { (&*UnsafeRef::into_raw(noderef)).thread };
            sched::unblock(thread);
        }
    }
}
