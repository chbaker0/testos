use super::pic;

use x86_64::structures::idt::ExceptionStackFrame;

fn handle_irq(irq: u8) {
    assert!(irq < 16);

    if irq == 7 || irq == 15 {
        if !pic::in_service(irq) {
            return;
        }
    }

    assert!(pic::in_service(irq));

    pic::eoi(irq, false);
}

pub extern "x86-interrupt" fn irq0_handler(_: &mut ExceptionStackFrame) {
    handle_irq(0);
}

pub extern "x86-interrupt" fn irq1_handler(_: &mut ExceptionStackFrame) {
    handle_irq(1);
}

pub extern "x86-interrupt" fn irq2_handler(_: &mut ExceptionStackFrame) {
    handle_irq(2);
}

pub extern "x86-interrupt" fn irq3_handler(_: &mut ExceptionStackFrame) {
    handle_irq(3);
}

pub extern "x86-interrupt" fn irq4_handler(_: &mut ExceptionStackFrame) {
    handle_irq(4);
}

pub extern "x86-interrupt" fn irq5_handler(_: &mut ExceptionStackFrame) {
    handle_irq(5);
}

pub extern "x86-interrupt" fn irq6_handler(_: &mut ExceptionStackFrame) {
    handle_irq(6);
}

pub extern "x86-interrupt" fn irq7_handler(_: &mut ExceptionStackFrame) {
    handle_irq(7);
}

pub extern "x86-interrupt" fn irq8_handler(_: &mut ExceptionStackFrame) {
    handle_irq(8);
}

pub extern "x86-interrupt" fn irq9_handler(_: &mut ExceptionStackFrame) {
    handle_irq(9);
}

pub extern "x86-interrupt" fn irq10_handler(_: &mut ExceptionStackFrame) {
    handle_irq(10);
}

pub extern "x86-interrupt" fn irq11_handler(_: &mut ExceptionStackFrame) {
    handle_irq(11);
}

pub extern "x86-interrupt" fn irq12_handler(_: &mut ExceptionStackFrame) {
    handle_irq(12);
}

pub extern "x86-interrupt" fn irq13_handler(_: &mut ExceptionStackFrame) {
    handle_irq(13);
}

pub extern "x86-interrupt" fn irq14_handler(_: &mut ExceptionStackFrame) {
    handle_irq(14);
}

pub extern "x86-interrupt" fn irq15_handler(_: &mut ExceptionStackFrame) {
    handle_irq(15);
}
