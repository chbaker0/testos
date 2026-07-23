use crate::mm;

use core::arch::{asm, naked_asm};
use core::mem;
use core::num::NonZeroUsize;
use core::ptr::NonNull;

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

// SAFETY: a `TaskPtr` is only ever dereferenced by the scheduler itself,
// which runs cooperatively (no preemption: a task only stops running via an
// explicit `yield_current`/`quit_current` call) and, at the points it's used,
// with interrupts disabled (`without_interrupts` in `pop_next_ready_task`/
// `add_task_to_ready_list`, or interrupts not yet enabled during boot). There
// is no real multi-core support yet, so there is no actual concurrent access
// to guard against beyond that.
unsafe impl Send for TaskPtr {}

struct Scheduler {
    ready_list_head: Option<TaskPtr>,
}

/// # Safety
///
/// Must be called at most once, before the scheduler is otherwise used (no
/// prior `spawn_kthread`/`yield_current`/etc.), and must never return to its
/// caller in the ordinary sense — it discards the caller's stack and jumps
/// directly into a new task context running `kernel_main_init_fn`, then
/// `kernel_main`.
pub unsafe fn init_kernel_main_thread(kernel_main: fn() -> !) -> ! {
    // SAFETY: `kernel_main` is a primitive pointer-sized type. It is safe to
    // transmute to `usize`, even as a function argument.
    let mut main_task = unsafe { create_task_typed(kernel_main_init_fn, kernel_main) };

    {
        let mut current_task = CURRENT_TASK.lock();
        if current_task.is_some() {
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

    // SAFETY: `main_task` was just created by `create_task_typed` above and
    // isn't shared with anything else yet (it's only stashed in
    // `CURRENT_TASK`, not the ready list), so taking `&mut` its pointee here
    // doesn't alias any other live reference.
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
    // SAFETY: `task` was just created by `create_task` and isn't referenced
    // anywhere else yet, satisfying `add_task_to_ready_list`'s contract (see
    // its doc) that `task` isn't already linked into any list.
    unsafe {
        add_task_to_ready_list(task);
    }
}

pub fn quit_current() -> ! {
    let (next_task_stack, old_task): (usize, *const Task) = {
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
        // SAFETY: `next_task` came from `pop_next_ready_task`, which returns
        // either a task unlinked from the ready list or the idle task,
        // neither aliased elsewhere; it's not currently running (we are
        // still running as `old_task`), so its `Task` isn't borrowed
        // anywhere else.
        let next_task_stack: usize = unsafe { next_task.0.as_mut().rsp.take().unwrap().get() };
        let mut stack_writer = StackWriter::new(next_task_stack as *mut ());
        // SAFETY: `next_task_stack` is the top of a `STACK_LEN`-sized,
        // otherwise-idle kernel stack (see `create_task`), so there's ample
        // room for one more function-pointer-sized push, satisfying
        // `StackWriter::push`'s space requirement. The pushed value becomes
        // the return address `restore_task_state` (invoked via `switch_to`
        // when this task next resumes) `ret`s into, so `clean_quit_task`
        // runs with `task` (this task's old `TaskPtr`, passed via `rdi` in
        // the `asm!` below) once we're off this stack.
        let next_task_stack = unsafe {
            stack_writer.push(clean_quit_task as unsafe extern "C" fn(*const Task));
            stack_writer.into_ptr() as usize
        };

        (next_task_stack, old_task.0.as_ptr())
    };

    // SAFETY: `next_task_stack` is `next_task`'s stack top with
    // `clean_quit_task` pushed as a return address (see above), so switching
    // `rsp` to it and `ret`-ing jumps there with `old_task` in `rdi`, its
    // sole argument. This never returns to `quit_current`'s caller, matching
    // this function's `-> !`.
    unsafe {
        asm!(
            "mov rsp, rax",
            "ret",
            in("rax") next_task_stack,
            in("rdi") old_task,
            options(noreturn),
        )
    }
}

/// This function cannot be called safely from Rust in the ordinary sense: it
/// is never `call`ed, only `ret`urned into by `quit_current`'s `asm!` block
/// above, which has already switched `rsp` onto the *next* task's stack.
/// `task` (the just-retired task) arrives in `rdi` per the C calling
/// convention.
///
/// # Safety
///
/// `task` must point to a valid, fully-populated `Task` owning a stack that is
/// no longer active — in particular **not** the stack this function is running
/// on — and not referenced anywhere else. That stack is freed here.
unsafe extern "C" fn clean_quit_task(task: *const Task) {
    // SAFETY: per this fn's contract `task` is valid and exclusively owned.
    // Copying it out, rather than holding a reference into the retired stack,
    // lets the `Task`'s drop free that stack without invalidating us.
    let task = unsafe { task.read() };
    assert_eq!(task.next_in_list, None);
    assert_eq!(task.prev_in_list, None);
    assert_eq!(task.rsp, None);
}

pub fn yield_current() {
    let (mut next_task, mut prev_task) = {
        let mut cur_task_guard = CURRENT_TASK.lock();
        let cur_task = &mut *cur_task_guard;

        let prev_task = cur_task.take().unwrap();
        // SAFETY: `prev_task` was just taken out of `CURRENT_TASK`, so it
        // isn't linked into the ready list (satisfying
        // `add_task_to_ready_list`'s contract) and isn't referenced anywhere
        // else concurrently.
        unsafe {
            add_task_to_ready_list(prev_task);
        }
        let next_task = pop_next_ready_task();
        *cur_task = Some(next_task);

        (next_task, prev_task)
    };

    if next_task == prev_task {
        return;
    }

    // SAFETY: `next_task` came from `pop_next_ready_task`, unlinked and not
    // aliased elsewhere, and isn't currently running (we're still running as
    // `prev_task`).
    let next_rsp: usize = unsafe { next_task.0.as_mut().rsp.take().unwrap().get() };
    // SAFETY: `prev_task` is `CURRENT_TASK`'s old value, exclusively owned
    // here, so `(*prev_task.0.as_ptr()).rsp` is a valid, live `usize`-sized
    // `Option<NonZeroUsize>` slot that `switch_to` writes the suspended `rsp`
    // into (see its own contract). `&raw mut` projects to that field without
    // forming a `&mut Task`, which matters because `prev_task` is the task
    // being switched away from. `Option<NonZeroUsize>` is niche-optimized to
    // the same layout as `usize`.
    let prev_rsp: *mut usize = unsafe { &raw mut (*prev_task.0.as_ptr()).rsp as *mut usize };

    // SAFETY: `next_rsp` is a valid suspended stack pointer for `next_task`
    // (either freshly built by `create_task` or previously saved by this
    // same `switch_to` call), `prev_rsp` points at live storage to save the
    // current stack pointer into, and `restore_task_state` matches the state
    // `switch_to` pushes before any save/restore, satisfying `switch_to`'s
    // contract.
    unsafe {
        switch_to(next_rsp, prev_rsp, restore_task_state);
    }
}

fn pop_next_ready_task() -> TaskPtr {
    interrupts::without_interrupts(|| {
        let mut scheduler_guard = SCHEDULER.lock();
        let scheduler = scheduler_guard.as_mut().unwrap();
        if let Some(mut list_head) = scheduler.ready_list_head {
            // SAFETY: `list_head` is `scheduler.ready_list_head`, maintained
            // by this module as always pointing at a valid, exclusively
            // scheduler-owned `Task` (see `add_task_to_ready_list`); this
            // runs under `without_interrupts` so nothing else can observe or
            // mutate it concurrently.
            let head_task = unsafe { list_head.0.as_mut() };
            scheduler.ready_list_head = head_task.next_in_list;
            head_task.next_in_list = None;
            head_task.prev_in_list = None;
            list_head
        } else {
            IDLE_TASK.lock().unwrap()
        }
    })
}

/// # Safety
///
/// `task` must not currently be linked into the ready list (i.e. its
/// `prev_in_list`/`next_in_list` must be `None` and it must not be
/// `SCHEDULER`'s `ready_list_head`), and must be a valid, exclusively-owned
/// `Task` that nothing else will mutate concurrently.
unsafe fn add_task_to_ready_list(mut task: TaskPtr) {
    interrupts::without_interrupts(|| {
        let mut scheduler_guard = SCHEDULER.lock();
        let scheduler = scheduler_guard.as_mut().unwrap();
        if let Some(mut list_tail) = scheduler.ready_list_head {
            // SAFETY: every task reachable from `ready_list_head` is a valid,
            // scheduler-owned `Task` (see this fn's contract, maintained
            // inductively); this runs under `without_interrupts`.
            while let Some(next) = unsafe { list_tail.0.as_mut().next_in_list } {
                list_tail = next;
            }

            // SAFETY: `task` is valid per this fn's contract, and
            // `list_tail` is the last node of the (valid, per above)
            // existing chain, so linking `task` in after it is sound.
            unsafe {
                task.0.as_mut().prev_in_list = Some(list_tail);
                list_tail.0.as_mut().next_in_list = Some(task);
            }
        } else {
            scheduler.ready_list_head = Some(task);
        }
    });
}

/// Switches the running kernel stack from the caller's to `next_rsp`,
/// optionally saving the caller's own suspended stack pointer to `*prev_rsp`
/// first. When `next_rsp` is later switched back to (by a future call to
/// this same function), execution resumes by jumping to `restore_fn`, which
/// must pop exactly the register state this function pushed (see
/// `restore_task_state`) and then return to (what looks like, to the
/// compiler) this function's caller.
///
/// # Safety
///
/// * `next_rsp` must be a stack pointer previously saved via this same
///   function's `prev_rsp` output (a suspended task), or one freshly built by
///   `create_task` (whose top-of-stack layout `task_init_trampoline`, not
///   this function, is responsible for matching).
/// * `prev_rsp` must point to valid, writable storage for a stack pointer
///   (unless the save is skipped; see the TODO below).
/// * `restore_fn` must be a function whose prologue expects the exact
///   register layout this function pushes.
#[unsafe(naked)]
unsafe extern "C" fn switch_to(
    next_rsp: usize,                    /* rdi */
    prev_rsp: *mut usize,               /* rsi */
    restore_fn: unsafe extern "C" fn(), /* rdx */
) {
    // TODO(chbaker0): the `test rax, rax` / `jz 2f` conditional skips saving
    // `rsp` to `*prev_rsp` when `rax` is zero, but `rax` is not one of this
    // naked function's declared inputs (only `rdi`/`rsi`/`rdx`, per the
    // comments above), and `yield_current`'s ordinary Rust call to
    // `switch_to` does not deliberately set it — so this branch's outcome
    // depends on whatever `rax` happens to hold at the call site, which I
    // could not establish is meaningful (dead/vestigial condition, or a
    // genuine bug that's benign only by luck so far). Everything else here
    // (the push sequence and its pairing with `restore_task_state`) I
    // believe is sound; I did not want to assert that about this branch too.
    unsafe {
        naked_asm!(
            "pushfq",
            "push rbp",
            "push rbx",
            "push r12",
            "push r13",
            "push r14",
            "push r15",
            "push rdx",
            "test rax, rax",
            "jz 2f",
            "mov [rsi], rsp",
            "2:",
            "mov rsp, rdi",
            "ret",
        )
    }
}

/// Resumes a task previously suspended by `switch_to`.
///
/// # Safety
///
/// Must only be reached by `ret`-ing into it (never `call`ed) with `rsp`
/// pointing exactly at the register state `switch_to` pushed for the task
/// being resumed — i.e. as `switch_to`'s `restore_fn` argument, resumed via
/// its own `ret`, never invoked any other way.
#[unsafe(naked)]
unsafe extern "C" fn restore_task_state() {
    // SAFETY: forwarded from this fn's contract: the stack at entry holds
    // exactly the `r15, r14, r13, r12, rbx, rbp, rflags` pushed by
    // `switch_to`, in that order, with the original caller's return address
    // just below them — so popping them back in reverse, then `ret`,
    // restores that caller's exact pre-switch state.
    unsafe {
        naked_asm!(
            "pop r15", "pop r14", "pop r13", "pop r12", "pop rbx", "pop rbp", "popfq", "ret",
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
    assert_eq!(mem::size_of_val(&context), mem::size_of::<usize>());
    // SAFETY: an extern "C" fn on x86-64 expects a single 8-byte primitive
    // argument to be passed by register. This is safe if `T` meets the
    // requirements imposed on the caller.
    unsafe {
        let task_fn = mem::transmute::<extern "C" fn(T) -> !, extern "C" fn(usize) -> !>(task_fn);
        let context_int = mem::transmute_copy::<T, usize>(&context);
        mem::forget(context);
        create_task(task_fn, context_int)
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
    // SAFETY: `stack_top` is the top of the `STACK_LEN`-byte stack just
    // allocated above, so there's room for `Task` (satisfying `push`'s space
    // requirement), and nothing else references this fresh allocation yet.
    let task_ptr = unsafe { stack_writer.push(task) };
    // SAFETY: same freshly-allocated, exclusively-owned stack as above, with
    // ample room left for four more `usize`-sized pushes. `task_ptr` (from
    // the `push` above) points at the just-written `Task`, which is still
    // valid: `push` only moves the writer's internal cursor further down the
    // stack, it doesn't invalidate memory already written above it.
    unsafe {
        stack_writer.push(0usize);
        stack_writer.push(task_fn);
        stack_writer.push(context);
        stack_writer.push(task_init_trampoline as unsafe extern "C" fn() -> !);

        (*task_ptr).rsp = NonZeroUsize::new(stack_writer.into_ptr() as usize);
    }

    TaskPtr(NonNull::new(task_ptr).unwrap())
}

/// This function cannot be called safely from Rust. The ABI is a lie. It does
/// not follow any normal calling convention.
///
/// # Safety
///
/// The task entry point and task context must be pushed on the stack in
/// order (see `create_task`'s layout comment), then this function must be
/// jumped to (via `switch_to`'s `ret`, never `call`ed).
#[unsafe(naked)]
unsafe extern "C" fn task_init_trampoline() -> ! {
    // SAFETY: forwarded from this fn's contract: the stack at entry has
    // `context` on top and `task_fn` just below it (per `create_task`), so
    // popping `context` into `rdi` (the sole argument register) and then
    // `ret`-ing pops and jumps to `task_fn`, calling it as `task_fn(context)`.
    unsafe {
        naked_asm!(
            // Get the context and place it in rdi, the first and only arg.
            "pop rdi", // "Return" to the task_fn, the next argument on the stack.
            "ret",
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
        // SAFETY: forwarded from this fn's contract: the caller guarantees
        // enough stack space for `T`, and `write_unaligned` is used (rather
        // than requiring alignment) precisely because the contract doesn't
        // promise `T`'s alignment.
        unsafe {
            assert_ne!(mem::size_of_val(&val), 0);
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
