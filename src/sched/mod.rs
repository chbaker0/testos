use crate::mm;

use core::arch::asm;
use core::mem;

use spin;

pub struct Task {
    /// Owned frames on which the task's kernel stack resides. This task's
    /// `Task` instance itself resides here.
    stack_frames: mm::OwnedFrameRange,

    // Scheduler info
    /// Per-task info required by the scheduler
    next_in_list: TaskPtr,
}

#[derive(Clone, Copy, Eq, PartialEq)]
pub struct TaskPtr(*mut Task);

unsafe impl Send for TaskPtr {}

pub struct Scheduler {
    ready_list_head: TaskPtr,
}

pub unsafe fn init_kernel_main_thread(kernel_main: extern "C" fn() -> !) -> ! {
    let main_task = create_task();
    {
        let mut current_task = CURRENT_TASK.lock();
        if current_task.0 != core::ptr::null_mut() {
            drop(current_task);
            panic!("current task existed while initializing tasks");
        }
        *current_task = main_task;
    }

    let stack_top: *mut () = main_task.0 as *mut ();

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

    TaskPtr(stack_ptr)
}

/// The currently running task. Null before the scheduling system is
/// initialized.
static CURRENT_TASK: spin::Mutex<TaskPtr> = spin::Mutex::new(TaskPtr(core::ptr::null_mut()));

pub const STACK_FRAMES_ORDER: usize = 2;
pub const STACK_FRAMES: usize = 2 << STACK_FRAMES_ORDER;

pub const STACK_LEN: usize = STACK_FRAMES * (mm::PAGE_SIZE.as_raw() as usize);
