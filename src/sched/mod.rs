use crate::mm;

use core::arch::asm;
use core::mem;
use core::num::NonZeroUsize;
use core::ptr::{null_mut, NonNull};

use spin;
use x86_64::instructions::interrupts;

pub struct Task {
    /// Owned frames on which the task's kernel stack resides. This task's
    /// `Task` instance itself resides here.
    stack_frames: mm::OwnedFrameRange,

    /// The last stack pointer, if the task is not currently running.
    rsp: Option<NonZeroUsize>,

    // Scheduler info
    prev_in_list: Option<TaskPtr>,
    next_in_list: Option<TaskPtr>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(transparent)]
pub struct TaskPtr(NonNull<Task>);

unsafe impl Send for TaskPtr {}

struct Scheduler {
    ready_list_head: Option<TaskPtr>,
}

pub unsafe fn init_kernel_main_thread(kernel_main: fn() -> !) -> ! {
    // SAFETY: `kernel_main` is a primitive pointer-sized type. It is safe to
    // transmute to `usize`, even as a function argument.
    let mut main_task = unsafe { create_task_typed(kernel_main_init_fn, kernel_main) };

    {
        let mut current_task = CURRENT_TASK.lock();
        if *current_task != None {
            drop(current_task);
            panic!("current task existed while initializing tasks");
        }
        *current_task = Some(main_task);
    }

    {
        *SCHEDULER.lock() = Some(Scheduler {
            ready_list_head: None,
        });
    }

    let stack_top: usize = unsafe { main_task.0.as_mut().rsp.take().unwrap().get() };

    // Discard the old stack, load the new one, and jump to
    // `kernel_main_init_fn`. This continues initialization once in a task
    // context.
    //
    // SAFETY: `stack_top` is a valid pointer to the top of the new stack.
    // Jumping to `kernel_main_init_fn` is safe because it never returns.
    unsafe {
        asm!(
            "mov rsp, {stack_top}",
            "ret",
            stack_top = in(reg) stack_top,
            options(noreturn),
        )
    }
}

pub fn spawn_kthread(task_fn: extern "C" fn(usize) -> !, context: usize) {
    let task = create_task(task_fn, context);
    unsafe {
        add_task_to_ready_list(task);
    }
}

pub fn quit_current() -> ! {
    {
        let mut cur_task_guard = CURRENT_TASK.lock();
        let cur_task = &mut *cur_task_guard;

        let old_task = cur_task.take().unwrap();

        // We can't clean up the current task on its own stack frame. Dropping
        // the `Task` object effectively invalidates our stack immediately,
        // which is fundamentally unsafe.
        //
        // Instead, we defer cleanup to the next task by pushing our cleanup
        // function to the top of its stack. This is OK because we know there is
        // always a next task: worst case, it's the idle task.
        let mut next_task = pop_next_ready_task();
        let next_task_stack: usize = unsafe { next_task.0.as_mut().rsp.take().unwrap().get() };
        let mut stack_writer = StackWriter::new(next_task_stack as *mut ());
        let next_task_stack = unsafe {
            stack_writer.push(clean_quit_task);
            stack_writer.into_ptr() as usize
        };

        unsafe {
            asm!(
                "mov rsp, rdi",
                "push {restore_task_state}",
                "push {clean_quit_task}",
                "ret",
                restore_task_state = sym restore_task_state,
                clean_quit_task = sym clean_quit_task,
                in("rdi") next_task_stack,
                in("rsi") old_task.0.as_ptr(),
                options(noreturn),
            )
        }
    }
}

unsafe extern "C" fn clean_quit_task(next_rsp: usize, task: TaskPtr) {
    // Read the value out of the task's stack so we can drop it safely (it
    // owns its own stack).
    let task = unsafe { task.0.as_ptr().read() };
    assert_eq!(task.next_in_list, None);
    assert_eq!(task.prev_in_list, None);
    assert_eq!(task.rsp, None);

    unsafe {
        asm!(
            "ret",
            in("rdi") next_rsp,
            options(noreturn),
        )
    }
}

pub fn yield_current() {
    let (next_task, prev_task) = {
        let mut cur_task_guard = CURRENT_TASK.lock();
        let cur_task = &mut *cur_task_guard;

        let prev_task = cur_task.take().unwrap();
        unsafe {
            add_task_to_ready_list(prev_task);
        }
        let next_task = pop_next_ready_task();
        *cur_task = Some(next_task);

        (next_task, prev_task)
    };

    unsafe {
        switch_to(next_task, Some(prev_task));
    }
}

fn pop_next_ready_task() -> TaskPtr {
    interrupts::without_interrupts(|| {
        let mut scheduler_guard = SCHEDULER.lock();
        let mut scheduler = scheduler_guard.as_mut().unwrap();
        if let Some(mut list_head) = scheduler.ready_list_head {
            let mut head_task = unsafe { list_head.0.as_mut() };
            scheduler.ready_list_head = head_task.next_in_list;
            head_task.next_in_list = None;
            head_task.prev_in_list = None;
            list_head
        } else {
            IDLE_TASK.lock().unwrap()
        }
    })
}

unsafe fn add_task_to_ready_list(mut task: TaskPtr) {
    interrupts::without_interrupts(|| {
        let mut scheduler_guard = SCHEDULER.lock();
        let mut scheduler = scheduler_guard.as_mut().unwrap();
        if let Some(mut list_tail) = scheduler.ready_list_head {
            while let Some(next) = unsafe { list_tail.0.as_mut().next_in_list } {
                list_tail = next;
            }

            unsafe {
                task.0.as_mut().prev_in_list = Some(list_tail);
                list_tail.0.as_mut().next_in_list = Some(task);
            }
        } else {
            scheduler.ready_list_head = Some(task);
        }
    });
}

unsafe extern "C" fn switch_to(mut next_task: TaskPtr, prev_task: Option<TaskPtr>) {
    let next_rsp = unsafe { next_task.0.as_mut().rsp.take().unwrap() };
    let prev_rsp: *mut NonZeroUsize = if let Some(mut prev_task) = prev_task {
        unsafe { &mut prev_task.0.as_mut().rsp as *mut Option<NonZeroUsize> as *mut NonZeroUsize }
    } else {
        null_mut()
    };
    unsafe {
        asm!(
            "pushfq",
            "push rbp",
            "push rbx",
            "push r12",
            "push r13",
            "push r14",
            "push r15",

            "test rax, rax",
            "jz 2",
            "mov [rax], rsp",
            "2:",
            "jmp [rip+{restore_task_state}]",

            restore_task_state = sym restore_task_state,
            in("rax") prev_rsp,
            in("rdi") next_rsp.get(),
            clobber_abi("C"),
        )
    }
}

#[naked]
unsafe extern "C" fn restore_task_state(next_rsp: usize) {
    unsafe {
        asm!(
            "mov rsp, rdi",
            "pop r15",
            "pop r14",
            "pop r13",
            "pop r12",
            "pop rbx",
            "pop rbp",
            "popfq",
            options(noreturn),
        )
    }
}

/// Convenience wrapper for `create_task` which takes a `usize`-sized
/// `context` (and panics otherwise).
///
/// # Safety
///
/// `T` must be a primitive type (such as a *const, *mut, or fn pointer). It
/// must have no alignment constraint stronger than `usize`.
unsafe fn create_task_typed<T>(task_fn: extern "C" fn(T) -> !, context: T) -> TaskPtr {
    // SAFETY: an extern "C" fn on x86-64 expects a single 8-byte primitive
    // argument to be passed by register. This is safe if `T` meets the
    // requirements imposed on the caller.
    unsafe {
        let task_fn = mem::transmute::<extern "C" fn(T) -> !, extern "C" fn(usize) -> !>(task_fn);
        let context = mem::transmute_copy::<T, usize>(&context);
        mem::forget(context);
        create_task(task_fn, context)
    }
}

/// Initialize a task stack, returning a pointer to the descriptor (which is
/// contained on the stack).
fn create_task(task_fn: extern "C" fn(usize) -> !, context: usize) -> TaskPtr {
    let task = Task {
        // Allocate 2^1 = 2 frames for the stack.
        stack_frames: mm::allocate_owned_frames(1).unwrap(),
        rsp: None,
        prev_in_list: None,
        next_in_list: None,
    };

    // For the stack pointer, simply use our direct mapping of physical to virtual memory.
    let stack_bottom: mm::VirtAddress =
        mm::phys_to_virt(task.stack_frames.frames().first().start());
    let stack_top = stack_bottom + mm::Length::from_raw(STACK_LEN as u64);

    // We write three things to the stack, from top downward:
    // 1. the Task instance (which is never accessed by the task),
    // 2. a 0usize, a null return address at the bottom of the call stack,
    // 3. the task_fn, which is called by task_init_trampoline,
    // 4. the context, which is passed by task_init_trampoline to task_fn, and
    // 5. task_init_trampoline which is returned to.
    let mut stack_writer = StackWriter::new(stack_top.as_mut_ptr());
    let task_ptr = unsafe { stack_writer.push(task) };
    unsafe {
        stack_writer.push(0usize);
        stack_writer.push(task_fn);
        stack_writer.push(context);
        stack_writer.push(task_init_trampoline);

        (*task_ptr).rsp = NonZeroUsize::new(stack_writer.into_ptr() as usize);
    }

    TaskPtr(NonNull::new(task_ptr).unwrap())
}

/// This function cannot be called safely from Rust. The ABI is a lie. It does
/// not follow any normal calling convention. The task entry point and task
/// context must be pushed on the stack in order, then this function must be
/// jumped to, not called.
#[naked]
unsafe extern "C" fn task_init_trampoline() -> ! {
    unsafe {
        asm!(
            // Get the context and place it in rdi, the first and only arg.
            "pop rdi",
            // "Return" to the task_fn, the next argument on the stack.
            "ret",
            options(noreturn),
        )
    }
}

#[allow(improper_ctypes_definitions)]
extern "C" fn kernel_main_init_fn(kernel_main: fn() -> !) -> ! {
    // Now we are in a task context. Set up the idle task.
    let idle_task = create_task(idle_task_fn, 0);
    *IDLE_TASK.lock() = Some(idle_task);

    kernel_main()
}

extern "C" fn idle_task_fn(_context: usize) -> ! {
    crate::halt_loop();
}

/// Helper to push values onto a stack, given a stack pointer.
struct StackWriter {
    ptr: *mut (),
}

impl StackWriter {
    fn new(ptr: *mut ()) -> StackWriter {
        StackWriter { ptr }
    }

    /// Push a value onto the stack. The pointer is moved down by
    /// `size_of::<T>()` and `val` is written to this location. The inner
    /// pointer is updated with the new stack "top". The pointer to the value is
    /// returned.
    ///
    /// The returned pointer's address is equal to the original pointer minus
    /// the sum size of all values pushed. It is up to the caller to ensure
    /// proper alignment for `T` if necessary.
    ///
    /// # Safety
    ///
    /// * there must be enough space on the stack to hold `T`.
    /// * the returned pointer may not be aligned, depending on the original
    ///   alignment and the sizes of values pushed before.
    unsafe fn push<T>(&mut self, val: T) -> *mut T {
        unsafe {
            // Get the stack pointer and offset it down by one T. This ignores
            // alignment requirements for T.
            let val_ptr = self.ptr.cast::<T>().sub(1);
            // Write the value. We don't know the alignment, so write unaligned.
            val_ptr.write_unaligned(val);
            // Update the stack pointer.
            self.ptr = val_ptr.cast();
            val_ptr
        }
    }

    /// Unwrap the inner stack pointer.
    fn into_ptr(self) -> *mut () {
        self.ptr
    }
}

/// The currently running task. Null before the scheduling system is
/// initialized.
static CURRENT_TASK: spin::Mutex<Option<TaskPtr>> = spin::Mutex::new(None);

/// The "idle task" which runs when no other task is ready.
static IDLE_TASK: spin::Mutex<Option<TaskPtr>> = spin::Mutex::new(None);

static SCHEDULER: spin::Mutex<Option<Scheduler>> = spin::Mutex::new(None);

pub const STACK_FRAMES_ORDER: usize = 2;
pub const STACK_FRAMES: usize = 2 << STACK_FRAMES_ORDER;

pub const STACK_LEN: usize = STACK_FRAMES * (mm::PAGE_SIZE.as_raw() as usize);
