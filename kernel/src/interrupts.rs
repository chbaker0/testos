use x86_64::structures::idt;

static mut IDT: idt::InterruptDescriptorTable = idt::InterruptDescriptorTable::new();

pub fn init() {
    unsafe {
        IDT.page_fault.set_handler_fn(page_fault_handler);
        IDT.double_fault.set_handler_fn(double_fault_handler);
        IDT.load();
    }
}

extern "x86-interrupt" fn page_fault_handler(
    _: &mut idt::ExceptionStackFrame,
    error_code: idt::PageFaultErrorCode) {
    let addr: u64;
    unsafe { asm!("mov %cr2, $0" : "=r"(addr)); }
    panic!("Page fault occurred on address {:x}: {:?}", addr, error_code);
}

extern "x86-interrupt" fn double_fault_handler(
    _: &mut idt::ExceptionStackFrame,
    _: u64) {
    unsafe { asm!("cli"); }
    panic!("Double fault");
}
