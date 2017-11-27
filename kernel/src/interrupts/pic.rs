use x86_64::instructions::port::{inb, outb};

const PIC1_CMD: u16 = 0x20;
const PIC1_DATA: u16 = 0x21;
const PIC2_CMD: u16 = 0xa0;
const PIC2_DATA: u16 = 0xa1;

const EOI: u8 = 0x20;

const ICW1_INIT: u8 = 0x10;
const ICW1_IC4: u8 = 0x01;

const ICW4_8086: u8 = 0x01;

const VECTOR_OFFSET: u8 = 32;

pub fn init() {
    unsafe {
        // ICW1
        outb(PIC1_CMD, ICW1_INIT | ICW1_IC4);
        outb(PIC2_CMD, ICW1_INIT | ICW1_IC4);

        // ICW2 (interrupt offsets)
        outb(PIC1_DATA, VECTOR_OFFSET);
        outb(PIC2_DATA, VECTOR_OFFSET+8);

        // ICW3
        outb(PIC1_DATA, 4);
        outb(PIC2_DATA, 2);

        // ICW4
        outb(PIC1_DATA, ICW4_8086);
        outb(PIC2_DATA, ICW4_8086);

        // Masks
        outb(PIC1_DATA, 0xff);
        outb(PIC2_DATA, 0xff);
    }
}

pub fn eoi(irq: u8, spurious: bool) {
    assert!(irq < 16);

    if irq >= 8 && !spurious {
        unsafe {
            outb(PIC2_CMD, EOI);
        }
    }

    else if !spurious || irq >= 8 {
        unsafe {
            outb(PIC1_CMD, EOI);
        }
    }
}

pub fn mask(mut irq: u8) {
    assert!(irq < 16);

    let port: u16;

    if irq < 8 {
        port = PIC1_DATA;
    } else {
        port = PIC2_DATA;
        irq -= 8;
    }

    unsafe {
        let old_mask = inb(port);
        outb(port, old_mask | (1 << irq));
    }
}

pub fn unmask(mut irq: u8) {
    assert!(irq < 16);

    let port: u16;

    if irq < 8 {
        port = PIC1_DATA;
    } else {
        port = PIC2_DATA;
        irq -= 8;
    }

    unsafe {
        let old_mask = inb(port);
        outb(port, old_mask & !(1 << irq));
    }
}
