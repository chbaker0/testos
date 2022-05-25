use crate::mm;

use core::arch::asm;
use core::mem;
use core::ptr::NonNull;

use spin;

pub struct Task {
    /// Owned frames on which the task's kernel stack resides. This task's
    /// `Task` instance itself resides here.
    stack_frames: mm::OwnedFrameRange,

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

pub unsafe fn init_kernel_main_thread(kernel_main: extern "C" fn() -> !) -> ! {
    // Set up idle task, which is the task of last resort when nothing else is
    // runnable.
    let idle_task = create_task();

    let main_task = create_task();

    {
        let mut current_task = CURRENT_TASK.lock();
        if *current_task != None {
            drop(current_task);
            panic!("current task existed while initializing tasks");
        }
        *current_task = Some(main_task);
    }

    let stack_top: *mut () = main_task.0.as_ptr() as *mut ();

    // Discard the old stack, load the new one, and jump to `kernel_main`.
    //
    // SAFETY: `stack_top` is a valid pointer to the top of the new stack.
    // Jumping to `kernel_main` is safe because it never returns.
    unsafe {
        asm!(
            "mov rax, {kernel_main}",
            "mov rsp, {stack_top}",
            "mov rbp, rsp",
            "jmp rax",
            kernel_main = in(reg) kernel_main,
            stack_top = in(reg) stack_top,
        );
    }

    unreachable!()
}

/// Initialize a task stack, returning a pointer to the descriptor (which is
/// contained on the stack).
fn create_task() -> TaskPtr {
    let task = Task {
        // Allocate 2^1 = 2 frames for the stack.
        stack_frames: mm::allocate_owned_frames(1).unwrap(),
        prev_in_list: None,
        next_in_list: None,
    };

    // For the stack pointer, simply use our direct mapping of physical to virtual memory.
    let stack_bottom: mm::VirtAddress =
        mm::phys_to_virt(task.stack_frames.frames().first().start());
    let stack_top = stack_bottom + mm::Length::from_raw(STACK_LEN as u64);
    let stack_ptr: *mut Task =
        unsafe { stack_top.as_mut_ptr::<Task>().sub(mem::size_of::<Task>()) };

    // Write the task descriptor to the top of the stack.
    let task: &mut Task = unsafe {
        stack_ptr.write(task);
        &mut *stack_ptr
    };

    TaskPtr(NonNull::new(stack_ptr).unwrap())
}

extern "C" fn idle_task_fn() -> ! {
    crate::halt_loop();
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
