use ::interrupts::set_irq_handler;
use ::x86_util::{inb, outb};

pub fn init() {
    unsafe {
        // Set channel 0 to lobyte/hibyte access mode and rate
        // generator operating mode.
        outb(0x43, 0b00_11_010_0);
        // Output our desired reload value.
        outb(0x40, PIT_RELOAD_VALUE as u8);
        outb(0x40, (PIT_RELOAD_VALUE >> 8) as u8);
    }

    // The PIT IRQ is 0. Set our handler for that IRQ.
    set_irq_handler(0, Some(timer_handler));
}

fn timer_handler() {
    // Do nothing for now.
}

// The counter value at which we want the PIT to generate an
// interrupt. The interrupt frequency is
// 1193182 / `PIT_RELOAD_VALUE`. For an interrupt about every
// 10 microseconds, we use a value of 20.
const PIT_RELOAD_VALUE: u16 = 20;
