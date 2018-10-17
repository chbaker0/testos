mod pic;
mod irq;

use x86_64;
use x86_64::structures::idt;

pub use irq::set_irq_handler;

static mut IDT: idt::InterruptDescriptorTable = idt::InterruptDescriptorTable::new();

pub fn init() {
    unsafe {
        IDT.page_fault.set_handler_fn(page_fault_handler);
        IDT.double_fault.set_handler_fn(double_fault_handler);
        IDT.general_protection_fault.set_handler_fn(general_protection_fault_handler);
        IDT[32].set_handler_fn(irq::irq0_handler);
        IDT[33].set_handler_fn(irq::irq1_handler);
        IDT[34].set_handler_fn(irq::irq2_handler);
        IDT[35].set_handler_fn(irq::irq3_handler);
        IDT[36].set_handler_fn(irq::irq4_handler);
        IDT[37].set_handler_fn(irq::irq5_handler);
        IDT[38].set_handler_fn(irq::irq6_handler);
        IDT[39].set_handler_fn(irq::irq7_handler);
        IDT[40].set_handler_fn(irq::irq8_handler);
        IDT[41].set_handler_fn(irq::irq9_handler);
        IDT[42].set_handler_fn(irq::irq10_handler);
        IDT[43].set_handler_fn(irq::irq11_handler);
        IDT[44].set_handler_fn(irq::irq12_handler);
        IDT[45].set_handler_fn(irq::irq13_handler);
        IDT[46].set_handler_fn(irq::irq14_handler);
        IDT[47].set_handler_fn(irq::irq15_handler);
        IDT.load();
    }

    // Initializes PIC with IRQs starting at interrupt 32. All IRQs are masked by default.
    pic::init();

    irq::init();

    for i in 0..15 {
        pic::unmask(i);
    }

    unsafe { asm!("sti"); }
}

extern "x86-interrupt" fn page_fault_handler(
    _: &mut idt::ExceptionStackFrame,
    error_code: idt::PageFaultErrorCode,
) {
    let addr: u64;
    unsafe {
        asm!("mov %cr2, $0" : "=r"(addr));
    }
    panic!(
        "Page fault occurred on address {:x}: {:?}",
        addr, error_code
    );
}

extern "x86-interrupt" fn double_fault_handler(_: &mut idt::ExceptionStackFrame, _: u64) {
    unsafe {
        asm!("cli");
    }
    panic!("Double fault");
}

extern "x86-interrupt" fn general_protection_fault_handler(
    _: &mut idt::ExceptionStackFrame,
    seg: u64) {
    panic!("GPF on segment {}", seg);
}
