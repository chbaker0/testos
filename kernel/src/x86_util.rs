pub unsafe fn outb(port: u16, value: u8) {
    asm!("outb %al, %dx" :: "{al}"(value), "{dx}"(port));
}

pub unsafe fn inb(port: u16) -> u8 {
    let value: u8;
    asm!("inb %dx, %al" : "={al}"(value) : "{dx}"(port));
    value
}
