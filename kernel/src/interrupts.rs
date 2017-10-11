use x86_64::structures::idt;

lazy_static! {
    static ref IDT: idt::Idt = {
        let mut idt = idt::Idt::new();
        idt.page_fault.set_handler_fn(page_fault_handler);
        idt
    };
}

pub fn init() {
    IDT.load();
}

extern "x86-interrupt" fn page_fault_handler(
    _: &mut idt::ExceptionStackFrame,
    _: idt::PageFaultErrorCode) {
    panic!("Page fault occurred.");
}
