use x86_64;
use x86_64::structures::idt;

lazy_static! {
    static ref IDT: idt::Idt = {
        let mut idt = idt::Idt::new();
        idt.page_fault.set_handler_fn(page_fault_handler);
        idt.double_fault.set_handler_fn(double_fault_handler);
        idt
    };
}

pub fn init() {
    IDT.load();
}

extern "x86-interrupt" fn page_fault_handler(
    _: &mut idt::ExceptionStackFrame,
    error_code: idt::PageFaultErrorCode) {
    let addr = x86_64::registers::control_regs::cr2();
    panic!("Page fault occurred on address {:x}: {:?}", addr, error_code);
}

extern "x86-interrupt" fn double_fault_handler(
    _: &mut idt::ExceptionStackFrame,
    _: u64) {
    unsafe { asm!("cli"); }
    panic!("Double fault");
}
