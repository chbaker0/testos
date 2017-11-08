use core::ops::DerefMut;
use core::sync::atomic::{AtomicU64, Ordering};

use alloc::boxed::Box;
use alloc::btree_map::BTreeMap;
use alloc::vec_deque::VecDeque;
use spin::Mutex;

use context::Context;

enum ThreadStatus {
    Running,
    Ready,
    Blocked,
}

struct ThreadInfo {
    id: u64,
    status: ThreadStatus,
    context: Context,
}

struct ThreadList {
    map: BTreeMap<u64, Box<ThreadInfo>>,
    next_id: u64,
}

impl ThreadList {
    fn new() -> ThreadList {
        ThreadList {
            map: BTreeMap::new(),
            next_id: 1,
        }
    }

    fn get(&self, id: u64) -> Option<&Box<ThreadInfo>> {
        self.map.get(&id)
    }

    fn get_mut(&mut self, id: u64) -> Option<&mut Box<ThreadInfo>> {
        self.map.get_mut(&id)
    }

    fn create_initial_thread(&mut self) -> u64 {
        let id = self.next_id;
        self.next_id += 1;

        let thread_info = Box::new(ThreadInfo {
            id: id,
            status: ThreadStatus::Running,
            context: Context::new_empty(),
        });

        assert!(self.map.insert(id, thread_info).is_none());

        id
    }

    fn create_thread(&mut self, stack_pages: u64, entry: extern fn() -> !) -> u64 {
        let id = self.next_id;
        self.next_id += 1;

        let thread_info = Box::new(ThreadInfo {
            id: id,
            status: ThreadStatus::Ready,
            context: Context::new(stack_pages, entry),
        });

        assert!(self.map.insert(id, thread_info).is_none());

        id
    }
}

lazy_static! {
    static ref THREADS: Mutex<ThreadList> = {
        Mutex::new(ThreadList::new())
    };

    static ref READY_QUEUE: Mutex<VecDeque<u64>> = {
        Mutex::new(VecDeque::new())
    };
}

static THREAD_ID: AtomicU64 = AtomicU64::new(0);

pub fn init() {
    assert!(THREAD_ID.load(Ordering::SeqCst) == 0);

    let initial_id = {
        let mut threads = THREADS.lock();
        threads.create_initial_thread()
    };

    THREAD_ID.store(initial_id, Ordering::SeqCst);
}

// Switches from current thread to new thread with ID
// `next_id`. Caller must ensure neither thread will be removed from
// the thread list.
unsafe fn switch_to(next_id: u64) {
    let cur_id = THREAD_ID.load(Ordering::SeqCst);

    let cur_ptr: *mut ThreadInfo;
    let next_ptr: *mut ThreadInfo;
    {
        let mut threads = THREADS.lock();
        cur_ptr = threads.get_mut(cur_id).unwrap().deref_mut() as *mut ThreadInfo;
        next_ptr = threads.get_mut(next_id).unwrap().deref_mut() as *mut ThreadInfo;
    };

    THREAD_ID.store(next_id, Ordering::SeqCst);

    (*cur_ptr).status = ThreadStatus::Ready;
    (*next_ptr).status = ThreadStatus::Running;
    (*cur_ptr).context.switch(&mut (*next_ptr).context);
}

const STACK_PAGES: u64 = 64;

pub fn spawn(entry: extern fn() -> !) -> u64 {
    let id = {
        let mut threads = THREADS.lock();
        threads.create_thread(STACK_PAGES, entry)
    };

    let mut ready_queue = READY_QUEUE.lock();
    ready_queue.push_back(id);

    id
}

pub fn yield_cur() {
    let cur_id = THREAD_ID.load(Ordering::SeqCst);

    let next_id = {
        let mut ready_queue = READY_QUEUE.lock();
        ready_queue.push_back(cur_id);
        ready_queue.pop_front().unwrap()
    };

    {
        let mut threads = THREADS.lock();
        threads.get_mut(cur_id).unwrap().status = ThreadStatus::Ready;
        threads.get_mut(cur_id).unwrap().status = ThreadStatus::Running;
    }

    unsafe { switch_to(next_id); }
}
