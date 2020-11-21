//! IDT management
//!
//! The interrupt descriptor table maps CPU interrupts to handlers.

use spin::Mutex;
use x86_64::instructions::interrupts::without_interrupts;
use x86_64::structures::idt::*;

// The wrapped InterruptDescriptorTable must never be dropped or moved.
static IDT: Mutex<InterruptDescriptorTable> = Mutex::new(InterruptDescriptorTable::new());

pub fn init() {
    without_interrupts(init_impl);
}

fn init_impl() {
    // Make sure we are only called once.
    static IS_INITIALIZED: core::sync::atomic::AtomicBool =
        core::sync::atomic::AtomicBool::new(false);
    assert!(!IS_INITIALIZED.swap(true, core::sync::atomic::Ordering::SeqCst));

    let mut idt = IDT.lock();

    // Only one GDT code selector is used in the kernel. These by default use
    // the current selector.
    idt.divide_error.set_handler_fn(divide_error_handler);
    idt.debug.set_handler_fn(debug_handler);
    idt.non_maskable_interrupt.set_handler_fn(nmi_handler);
    idt.breakpoint.set_handler_fn(breakpoint_handler);
    idt.overflow.set_handler_fn(overflow_handler);
    idt.bound_range_exceeded
        .set_handler_fn(bound_range_exceeded_handler);
    idt.invalid_opcode.set_handler_fn(invalid_opcode_handler);
    idt.device_not_available
        .set_handler_fn(device_not_available_handler);
    idt.double_fault.set_handler_fn(double_fault_handler);
    idt[9].set_handler_fn(unrecognized_exception_handler);
    idt.invalid_tss.set_handler_fn(invalid_tss_handler);
    idt.segment_not_present
        .set_handler_fn(segment_not_present_handler);
    idt.stack_segment_fault
        .set_handler_fn(stack_segment_fault_handler);
    idt.general_protection_fault
        .set_handler_fn(general_protection_fault_handler);
    idt.page_fault.set_handler_fn(page_fault_handler);
    // Entry 15 is reserved
    idt.x87_floating_point
        .set_handler_fn(x87_floating_point_handler);
    idt.alignment_check.set_handler_fn(alignment_check_handler);
    idt.machine_check.set_handler_fn(machine_check_handler);
    idt.simd_floating_point
        .set_handler_fn(simd_floating_point_handler);
    idt.virtualization.set_handler_fn(virtualization_handler);
    // Entries 21..30 are reserved
    idt.security_exception
        .set_handler_fn(security_exception_handler);
    // Entry 31 is reserved

    for i in 32..256 {
        idt[i] = Entry::missing();
    }

    unsafe {
        // This is OK since IDT_RAW never moves and is never destroyed.
        idt.load_unsafe();
    }
}

pub unsafe fn install_interrupt_handler(num: u8, maybe_handler: Option<HandlerFunc>) {
    without_interrupts(|| {
        let mut idt = IDT.lock();
        if let Some(handler) = maybe_handler {
            idt[num as usize].set_handler_fn(handler);
        } else {
            idt[num as usize] = Entry::missing();
        }
    });
}

// Default exception handlers
extern "x86-interrupt" fn divide_error_handler(stack_frame: &mut InterruptStackFrame) {
    panic!("divide error 0 {:?}", stack_frame);
}

extern "x86-interrupt" fn debug_handler(stack_frame: &mut InterruptStackFrame) {
    panic!("debug 1 {:?}", stack_frame);
}

extern "x86-interrupt" fn nmi_handler(stack_frame: &mut InterruptStackFrame) {
    panic!("NMI 2 {:?}", stack_frame);
}

extern "x86-interrupt" fn breakpoint_handler(stack_frame: &mut InterruptStackFrame) {
    panic!("breakpoint 3 {:?}", stack_frame);
}

extern "x86-interrupt" fn overflow_handler(stack_frame: &mut InterruptStackFrame) {
    panic!("overflow 4 {:?}", stack_frame);
}

extern "x86-interrupt" fn bound_range_exceeded_handler(stack_frame: &mut InterruptStackFrame) {
    panic!("bound range exceeded 5 {:?}", stack_frame);
}

extern "x86-interrupt" fn invalid_opcode_handler(stack_frame: &mut InterruptStackFrame) {
    panic!("invalid opcode 6 {:?}", stack_frame);
}

extern "x86-interrupt" fn device_not_available_handler(stack_frame: &mut InterruptStackFrame) {
    panic!("device not available 7 {:?}", stack_frame);
}

extern "x86-interrupt" fn double_fault_handler(
    stack_frame: &mut InterruptStackFrame,
    _error_code: u64,
) -> ! {
    panic!("double fault 8 {:?}", stack_frame);
}

extern "x86-interrupt" fn invalid_tss_handler(
    stack_frame: &mut InterruptStackFrame,
    error_code: u64,
) {
    panic!("invalid TSS 10 {} {:?}", error_code, stack_frame);
}

extern "x86-interrupt" fn segment_not_present_handler(
    stack_frame: &mut InterruptStackFrame,
    error_code: u64,
) {
    panic!("segment not present 11 {} {:?}", error_code, stack_frame);
}

extern "x86-interrupt" fn stack_segment_fault_handler(
    stack_frame: &mut InterruptStackFrame,
    error_code: u64,
) {
    panic!("stack segment fault 12 {} {:?}", error_code, stack_frame);
}

extern "x86-interrupt" fn general_protection_fault_handler(
    stack_frame: &mut InterruptStackFrame,
    error_code: u64,
) {
    panic!(
        "general protection fault 13 {} {:?}",
        error_code, stack_frame
    );
}

extern "x86-interrupt" fn page_fault_handler(
    stack_frame: &mut InterruptStackFrame,
    error_code: PageFaultErrorCode,
) {
    panic!("page fault 14 {:?} {:?}", error_code, stack_frame);
}

extern "x86-interrupt" fn x87_floating_point_handler(stack_frame: &mut InterruptStackFrame) {
    panic!("x87 floating point 16 {:?}", stack_frame);
}

extern "x86-interrupt" fn alignment_check_handler(
    stack_frame: &mut InterruptStackFrame,
    _error_code: u64,
) {
    panic!("alignment check 17 {:?}", stack_frame);
}

extern "x86-interrupt" fn machine_check_handler(stack_frame: &mut InterruptStackFrame) -> ! {
    panic!("machine check 18 {:?}", stack_frame);
}

extern "x86-interrupt" fn simd_floating_point_handler(stack_frame: &mut InterruptStackFrame) {
    panic!("SIMD floating point 19 {:?}", stack_frame);
}

extern "x86-interrupt" fn virtualization_handler(stack_frame: &mut InterruptStackFrame) {
    panic!("virtualization 20 {:?}", stack_frame);
}

extern "x86-interrupt" fn security_exception_handler(
    stack_frame: &mut InterruptStackFrame,
    _error_code: u64,
) {
    panic!("security exception 30 {:?}", stack_frame);
}

extern "x86-interrupt" fn unrecognized_exception_handler(stack_frame: &mut InterruptStackFrame) {
    panic!("unrecognized exception {:?}", stack_frame);
}
