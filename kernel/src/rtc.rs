use interrupts;
use x86_util::{inb, outb};

/// Rate controls the interrupt frequency.
/// You can calculate the frequency with `frequency =  32768 >> (rate-1)`.
/// Rate must be between 3 and 15 inclusice.
const RATE: u8 = 6;

/// Struct with values from the Real Time Clock.
#[derive(Copy, Clone)]
pub struct RtcTime {
    pub seconds: u8,
    pub minutes: u8,
    pub hours: u8,
    pub day_of_week: u8,
    pub date_of_month: u8,
    pub month: u8,
    pub year: u8,
    pub count: u64,
}

static TIME: spin::Mutex<RtcTime> = spin::Mutex::new(RtcTime {
    seconds: 0,
    minutes: 0,
    hours: 0,
    day_of_week: 0,
    date_of_month: 0,
    month: 0,
    year: 0,
    count: 0,
});

pub fn init() {
    assert!(RATE >= 3 && RATE <= 15);

    interrupts::set_irq_handler(8, Some(rtc_interrupt_handler));

    unsafe {
        enable_irq8();
    }
}

pub fn get_time() -> RtcTime {
    let return_time: RtcTime;
    unsafe {
        asm!("cli");
        let time = TIME.lock();
        return_time = *time;
        asm!("sti");
    }
    return_time
}

/// Handler for the RTC Update Interrupt.
/// The default is for the interrupt to fire every second.
fn rtc_interrupt_handler() {
    unsafe {
        // The b register sets 24 hour mode and bcd/binary mode.
        outb(0x70, 0x8B);
        let reg_b: u8 = inb(0x71);
        let bcd: bool = reg_b & 0x04 == 0;

        outb(0x70, 0x00);
        let seconds = normalize(inb(0x71), bcd);
        outb(0x70, 0x02);
        let minutes = normalize(inb(0x71), bcd);
        outb(0x70, 0x04);
        let hours = normalize(inb(0x71), bcd);
        outb(0x70, 0x06);
        let day_of_week = normalize(inb(0x71), bcd);
        outb(0x70, 0x07);
        let date_of_month = normalize(inb(0x71), bcd);
        outb(0x70, 0x08);
        let month = normalize(inb(0x71), bcd);
        outb(0x70, 0x09);
        let year = normalize(inb(0x71), bcd);

        // Register c must be read after each interrupt or another will not occur.
        outb(0x70, 0x0C);
        inb(0x71);
    }

    // This is separate from reading the values in order to disable interrupts
    // for the least amount of time.
    unsafe {
        asm!("cli");
    }

    let mut time = TIME.lock();
    time.seconds = seconds;
    time.minutes = minutes;
    time.hours = hours;
    time.day_of_week = day_of_week;
    time.date_of_month = date_of_month;
    time.year = year;

    unsafe {
        asm!("sti");
    }
}

/// The values retreived from the RTC might be in BCD format.
/// If so, This will convery it to binary format.
fn normalize(value: u8, binary_coded_decimal: bool) -> u8 {
    if binary_coded_decimal {
        ((value & 0xF0) >> 1) + ((value & 0xF0) >> 3) + (value & 0xF)
    } else {
        value
    }
}

unsafe fn enable_irq8() {
    nmi_disable();

    // Turn on periodic interrupts to IRQ8.
    outb(0x70, 0x8B);
    let prev: u8 = inb(0x71);
    outb(0x70, 0x8B);
    outb(0x71, prev | 0x40);

    // Set the rate of the periodic interrupt.
    outb(0x70, 0x8A);
    let prev: u8 = inb(0x71);
    outb(0x70, 0x8A);
    outb(0x71, (prev & 0xF0) | (RATE & 0x0F));

    nmi_enable();
}

unsafe fn nmi_enable() {
    asm!("sti");
    outb(0x70, inb(0x70) & 0x7F);
}

unsafe fn nmi_disable() {
    asm!("cli");
    outb(0x70, inb(0x70) | 0x80);
}
