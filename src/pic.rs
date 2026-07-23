//! x86 PIC utilities

use spin::Mutex;
use x86_64::instructions::interrupts::without_interrupts;
use x86_64::instructions::port::*;
use x86_64::structures::idt::InterruptStackFrame;

use crate::idt::install_interrupt_handler;

pub type IrqHandlerFunc = fn(stack: InterruptStackFrame);

struct PicRegs {
    cmd_1: PortWriteOnly<u8>,
    cmd_2: PortWriteOnly<u8>,
    data_1: Port<u8>,
    data_2: Port<u8>,
}

static PIC_REGS: Mutex<PicRegs> = Mutex::new(PicRegs {
    // Commands can be written to each PIC's command port, e.g. to initialize or
    // to acknowledge an IRQ.
    cmd_1: PortWriteOnly::new(0x20),
    cmd_2: PortWriteOnly::new(0xa0),
    // Some commands must be followed up by data writes. When no command is
    // active, each PIC's data port reads/writes its IRQ mask. If bit N is set
    // in PIC 1's mask, then IRQ N will not be sent to the CPU. Likewise for PIC
    // 2 and IRQ N+8.
    data_1: Port::new(0x21),
    data_2: Port::new(0xa1),
});

/// # Safety
///
/// Interrupts must be disabled before this is called, so a spurious IRQ can't
/// arrive mid-sequence before the handlers below are installed. It is safe to
/// enable interrupts after `init()` returns.
pub unsafe fn init() {
    // SAFETY: forwarded from this fn's contract; `without_interrupts` itself
    // doesn't add any further requirement.
    without_interrupts(|| unsafe { init_impl() });
}

/// # Safety
///
/// Same contract as `init` (interrupts disabled on entry); this is only ever
/// called from `init` via `without_interrupts`.
unsafe fn init_impl() {
    let mut pic_regs = PIC_REGS.lock();

    // SAFETY: `pic_regs`' ports are the standard, fixed PIC command/data
    // ports (see `PIC_REGS`'s initializer), and this is the standard PIC
    // initialization sequence (ICW1-4) run once with interrupts disabled per
    // this fn's contract, so nothing else touches these ports concurrently.
    // The `install_interrupt_handler` calls use vectors
    // `IRQ_INTERRUPT_OFFSET..IRQ_INTERRUPT_OFFSET+15` (32..=47), which don't
    // overlap the CPU-reserved 0..=31 range `idt::install_interrupt_handler`
    // requires callers to avoid.
    unsafe {
        // Do the magic
        pic_regs.cmd_1.write(0x11);
        pic_regs.cmd_2.write(0x11);
        pic_regs.data_1.write(IRQ_INTERRUPT_OFFSET);
        pic_regs.data_2.write(IRQ_INTERRUPT_OFFSET + IRQS_PER_PIC);
        pic_regs.data_1.write(4);
        pic_regs.data_2.write(2);
        pic_regs.data_1.write(1);
        pic_regs.data_2.write(1);

        // Mask all interrupts
        pic_regs.data_1.write(0b11111111);
        pic_regs.data_2.write(0b11111111);

        install_interrupt_handler(IRQ_INTERRUPT_OFFSET, Some(handle_irq0));
        install_interrupt_handler(IRQ_INTERRUPT_OFFSET + 1, Some(handle_irq1));
        install_interrupt_handler(IRQ_INTERRUPT_OFFSET + 2, Some(handle_irq2));
        install_interrupt_handler(IRQ_INTERRUPT_OFFSET + 3, Some(handle_irq3));
        install_interrupt_handler(IRQ_INTERRUPT_OFFSET + 4, Some(handle_irq4));
        install_interrupt_handler(IRQ_INTERRUPT_OFFSET + 5, Some(handle_irq5));
        install_interrupt_handler(IRQ_INTERRUPT_OFFSET + 6, Some(handle_irq6));
        install_interrupt_handler(IRQ_INTERRUPT_OFFSET + 7, Some(handle_irq7));
        install_interrupt_handler(IRQ_INTERRUPT_OFFSET + 8, Some(handle_irq8));
        install_interrupt_handler(IRQ_INTERRUPT_OFFSET + 9, Some(handle_irq9));
        install_interrupt_handler(IRQ_INTERRUPT_OFFSET + 10, Some(handle_irq10));
        install_interrupt_handler(IRQ_INTERRUPT_OFFSET + 11, Some(handle_irq11));
        install_interrupt_handler(IRQ_INTERRUPT_OFFSET + 12, Some(handle_irq12));
        install_interrupt_handler(IRQ_INTERRUPT_OFFSET + 13, Some(handle_irq13));
        install_interrupt_handler(IRQ_INTERRUPT_OFFSET + 14, Some(handle_irq14));
        install_interrupt_handler(IRQ_INTERRUPT_OFFSET + 15, Some(handle_irq15));
    }
}

pub fn install_irq_handler(irq_num: u8, maybe_handler: Option<IrqHandlerFunc>) {
    assert!(irq_num < IRQS_PER_PIC * 2);

    without_interrupts(|| {
        {
            let mut handlers = IRQ_HANDLERS.lock();
            if let Some(handler) = maybe_handler {
                assert!(handlers[irq_num as usize].is_none());
                handlers[irq_num as usize] = Some(handler);
            } else {
                handlers[irq_num as usize] = None;
            }
        }

        let should_mask_irq = maybe_handler.is_none();
        let irq_chip = if irq_num < 8 { 0 } else { 1 };
        let irq_line = irq_num - 8 * irq_chip;

        let mut pic_regs = PIC_REGS.lock();
        if irq_chip == 0 {
            // SAFETY: `irq_line` is `irq_num - 8 * irq_chip` with
            // `irq_num < IRQS_PER_PIC * 2` (asserted above) and `irq_chip == 0`
            // here, so `irq_line < 8`, satisfying `set_mask`'s contract for
            // `data_1` (PIC 1's 8-bit mask port).
            unsafe {
                set_mask(&mut pic_regs.data_1, irq_line, should_mask_irq);
            }
        } else {
            // SAFETY: as above, but `irq_chip == 1` here so `irq_line` is
            // `irq_num - 8`, still `< 8`, satisfying the same contract for
            // `data_2` (PIC 2's mask port).
            unsafe {
                set_mask(&mut pic_regs.data_2, irq_line, should_mask_irq);
            }
        }
    });
}

/// # Safety
///
/// `data_port` must be one of the two PIC data ports (`PIC_REGS.data_1` or
/// `data_2`), and `irq_line` must be `< 8` (a bit index into that PIC's 8-bit
/// mask register).
unsafe fn set_mask(data_port: &mut Port<u8>, irq_line: u8, set: bool) {
    // SAFETY: forwarded from this fn's contract: `data_port` is a PIC data
    // port, which reads back the current IRQ mask when no command is active.
    let old_mask = unsafe { data_port.read() };
    let new_mask = if set {
        old_mask | (1 << irq_line)
    } else {
        old_mask & !(1 << irq_line)
    };

    // SAFETY: same port as the read above; writing it updates the mask.
    unsafe {
        data_port.write(new_mask);
    }
}

// For various reasons, an IRQ might be invalid in which case we shouldn't
// respond to the PIC. Only IRQs 7 and 15 may be spurious; in this case, we must
// ask the PIC which IRQs are currently in service.
fn is_spurious(irq_num: u8) -> bool {
    if irq_num != 7 && irq_num != 15 {
        return false;
    }

    let mut pic_regs = PIC_REGS.lock();
    let isr = if irq_num == 7 {
        // SAFETY: `cmd_1`/`data_1` are PIC 1's fixed command/data ports;
        // issuing the "read ISR" command then reading the data port back is
        // the standard sequence for querying the in-service register.
        unsafe {
            pic_regs.cmd_1.write(PIC_COMMAND_READ_ISR);
            pic_regs.data_1.read()
        }
    } else {
        // SAFETY: as above, but PIC 2's `cmd_2`/`data_2` ports.
        unsafe {
            pic_regs.cmd_2.write(PIC_COMMAND_READ_ISR);
            pic_regs.data_2.read()
        }
    };

    let is_spurious = isr & 0b10000000 != 0;

    // If it's spurious, we shouldn't issue an EOI to the originating PIC.
    // However, if the secondary PIC sent the spurious IRQ (i.e. IRQ 15), we
    // must still send EOI to the primary PIC.
    if irq_num == 15 {
        // SAFETY: `cmd_1` is PIC 1's fixed command port; writing the EOI
        // command there is how the primary PIC is acknowledged.
        unsafe {
            pic_regs.cmd_1.write(PIC_COMMAND_ACKNOWLEDGE_IRQ);
        }
    }

    is_spurious
}

fn acknowledge_irq(irq_num: u8) {
    let mut pic_regs = PIC_REGS.lock();

    // SAFETY: `cmd_1`/`cmd_2` are the fixed PIC 1/2 command ports; writing
    // the EOI command is the standard end-of-interrupt sequence (PIC 2 first
    // when the IRQ came from it, then always PIC 1, since PIC 2 is cascaded
    // through PIC 1).
    unsafe {
        if irq_num >= 8 {
            pic_regs.cmd_2.write(PIC_COMMAND_ACKNOWLEDGE_IRQ);
        }

        pic_regs.cmd_1.write(PIC_COMMAND_ACKNOWLEDGE_IRQ);
    }
}

static IRQ_HANDLERS: Mutex<[Option<IrqHandlerFunc>; 16]> = Mutex::new([None; 16]);

// Internal IRQ handlers
fn handle_irq(irq_num: u8, stack: InterruptStackFrame) {
    without_interrupts(|| {
        if is_spurious(irq_num) {
            return;
        }

        {
            let handlers = IRQ_HANDLERS.lock();
            if let Some(handler) = handlers[irq_num as usize] {
                handler(stack);
            } else {
                panic!("Unhandled IRQ {} received", irq_num);
            }
        }

        acknowledge_irq(irq_num);
    });
}

const PIC_COMMAND_READ_ISR: u8 = 0x0b;
const PIC_COMMAND_ACKNOWLEDGE_IRQ: u8 = 0x20;

extern "x86-interrupt" fn handle_irq0(stack: InterruptStackFrame) {
    handle_irq(0, stack);
}

extern "x86-interrupt" fn handle_irq1(stack: InterruptStackFrame) {
    handle_irq(1, stack);
}

extern "x86-interrupt" fn handle_irq2(stack: InterruptStackFrame) {
    handle_irq(2, stack);
}

extern "x86-interrupt" fn handle_irq3(stack: InterruptStackFrame) {
    handle_irq(3, stack);
}

extern "x86-interrupt" fn handle_irq4(stack: InterruptStackFrame) {
    handle_irq(4, stack);
}

extern "x86-interrupt" fn handle_irq5(stack: InterruptStackFrame) {
    handle_irq(5, stack);
}

extern "x86-interrupt" fn handle_irq6(stack: InterruptStackFrame) {
    handle_irq(6, stack);
}

extern "x86-interrupt" fn handle_irq7(stack: InterruptStackFrame) {
    handle_irq(7, stack);
}

extern "x86-interrupt" fn handle_irq8(stack: InterruptStackFrame) {
    handle_irq(8, stack);
}

extern "x86-interrupt" fn handle_irq9(stack: InterruptStackFrame) {
    handle_irq(9, stack);
}

extern "x86-interrupt" fn handle_irq10(stack: InterruptStackFrame) {
    handle_irq(10, stack);
}

extern "x86-interrupt" fn handle_irq11(stack: InterruptStackFrame) {
    handle_irq(11, stack);
}

extern "x86-interrupt" fn handle_irq12(stack: InterruptStackFrame) {
    handle_irq(12, stack);
}

extern "x86-interrupt" fn handle_irq13(stack: InterruptStackFrame) {
    handle_irq(13, stack);
}

extern "x86-interrupt" fn handle_irq14(stack: InterruptStackFrame) {
    handle_irq(14, stack);
}

extern "x86-interrupt" fn handle_irq15(stack: InterruptStackFrame) {
    handle_irq(15, stack);
}

// The desired CPU interrupt number for the first IRQ
pub const IRQ_INTERRUPT_OFFSET: u8 = 32;

// The number of IRQs serviced by each of the two PICs
const IRQS_PER_PIC: u8 = 8;
