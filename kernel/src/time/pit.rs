use spin::Mutex;

use interrupts::set_irq_handler;
use x86_util::{inb, outb};

use super::TickSource;

/// A `TickSource` adapter for the programmable interval timer.
pub struct Pit {
    data: spin::Mutex<PitData>,
}

struct PitData {
    ticks: u64,
    handler: Option<fn(u64)>,
}

impl TickSource for Pit {
    fn approx_ticks_per_second(&self) -> u64 {
        1193182 / PIT_RELOAD_VALUE as u64
    }

    fn set_tick_handler(&mut self, tick_handler: fn(u64)) {
        unsafe {
            asm!("cli");
        }

        {
            let mut data = self.data.lock();
            data.handler = Some(tick_handler);
        }

        unsafe {
            asm!("sti");
        }
    }

    fn get_ticks(&self) -> u64 {
        let ticks;

        unsafe {
            asm!("cli");
        }

        {
            let mut data = self.data.lock();
            ticks = data.ticks;
        }

        unsafe {
            asm!("sti");
        }

        ticks
    }
}

static PIT: Pit = Pit {
    data: spin::Mutex::new(PitData {
        ticks: 0,
        handler: None,
    }),
};

pub fn init() {
    // The PIT IRQ is 0. Set our handler for that IRQ.
    set_irq_handler(0, Some(timer_handler));

    unsafe {
        // Set channel 0 to lobyte/hibyte access mode and rate
        // generator operating mode.
        outb(0x43, 0b00_11_010_0);
        // Output our desired reload value.
        outb(0x40, PIT_RELOAD_VALUE as u8);
        outb(0x40, (PIT_RELOAD_VALUE >> 8) as u8);
    }
}

fn timer_handler() {
    let now_ticks;
    let maybe_handler;

    {
        unsafe {
            asm!("cli");
        }

        let mut pit_data = PIT.data.lock();
        pit_data.ticks += 1;

        now_ticks = pit_data.ticks;
        maybe_handler = pit_data.handler;

        unsafe {
            asm!("sti");
        }
    }

    match maybe_handler {
        Some(handler) => handler(now_ticks),
        None => (),
    };
}

// The counter value at which we want the PIT to generate an
// interrupt. The interrupt frequency is
// 1193182 / `PIT_RELOAD_VALUE`. For an interrupt about every
// 10 microseconds, we use a value of 20.
const PIT_RELOAD_VALUE: u16 = 1193;
