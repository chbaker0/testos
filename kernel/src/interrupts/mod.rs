mod pic;
mod apic;
mod irq;

use x86_64;
use x86_64::structures::idt;

lazy_static! {
    static ref IDT: idt::Idt = {
        let mut idt = idt::Idt::new();
        idt.page_fault.set_handler_fn(page_fault_handler);
        idt.double_fault.set_handler_fn(double_fault_handler);
        idt[32].set_handler_fn(irq::irq0_handler);
        idt[33].set_handler_fn(irq::irq1_handler);
        idt[34].set_handler_fn(irq::irq2_handler);
        idt[35].set_handler_fn(irq::irq3_handler);
        idt[36].set_handler_fn(irq::irq4_handler);
        idt[37].set_handler_fn(irq::irq5_handler);
        idt[38].set_handler_fn(irq::irq6_handler);
        idt[39].set_handler_fn(irq::irq7_handler);
        idt[40].set_handler_fn(irq::irq8_handler);
        idt[41].set_handler_fn(irq::irq9_handler);
        idt[42].set_handler_fn(irq::irq10_handler);
        idt[43].set_handler_fn(irq::irq11_handler);
        idt[44].set_handler_fn(irq::irq12_handler);
        idt[45].set_handler_fn(irq::irq13_handler);
        idt[46].set_handler_fn(irq::irq14_handler);
        idt[47].set_handler_fn(irq::irq15_handler);
        idt
    };
}

pub fn init() {
    IDT.load();

    // Initializes PIC with IRQs starting at interrupt 32. All IRQs are masked by default.
    pic::init();

    unsafe { asm!("sti"); }
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
