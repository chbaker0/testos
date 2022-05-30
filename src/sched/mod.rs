use crate::mm;

use core::arch::asm;
use core::mem;
use core::num::NonZeroUsize;
use core::ptr::NonNull;

use spin;

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

#[derive(Clone, Copy, Eq, PartialEq)]
#[repr(transparent)]
pub struct TaskPtr(NonNull<Task>);

unsafe impl Send for TaskPtr {}

pub struct Scheduler {
    ready_list_head: TaskPtr,
}

pub unsafe fn init_kernel_main_thread(kernel_main: fn() -> !) -> ! {
    // SAFETY: `kernel_main` is a primitive pointer-sized type. It is safe to
    // transmute to `usize`, even as a function argument.
    let main_task = unsafe { create_task_typed(kernel_main_init_fn, kernel_main) };

    {
        let mut current_task = CURRENT_TASK.lock();
        if *current_task != None {
            drop(current_task);
            panic!("current task existed while initializing tasks");
        }
        *current_task = Some(main_task);
    }

    let stack_top: usize = unsafe { main_task.0.as_ref().rsp.unwrap().get() };

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
    // 2. the task_fn, which is called by task_init_trampoline,
    // 3. the context, which is passed by task_init_trampoline to task_fn, and
    // 4. task_init_trampoline which is returned to.
    let mut stack_writer = StackWriter::new(stack_top.as_mut_ptr());
    let task_ptr = unsafe { stack_writer.push(task) };
    unsafe {
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

// static SCHEDULER: spin::Mutex<Option<Scheduler>> = spin::Mutex::new(None);

pub const STACK_FRAMES_ORDER: usize = 2;
pub const STACK_FRAMES: usize = 2 << STACK_FRAMES_ORDER;

pub const STACK_LEN: usize = STACK_FRAMES * (mm::PAGE_SIZE.as_raw() as usize);
