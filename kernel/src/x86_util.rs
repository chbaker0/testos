use x86_64::registers::rflags;

pub unsafe fn outb(port: u16, value: u8) {
    asm!("outb %al, %dx" :: "{al}"(value), "{dx}"(port));
}

pub unsafe fn inb(port: u16) -> u8 {
    let value: u8;
    asm!("inb %dx, %al" : "={al}"(value) : "{dx}"(port));
    value
}

/// Disables interrupts while in scope. When dropped, this resets
/// rflags to their original state.
pub struct ScopedInterruptDisabler {
    saved_flags: u64,
}

impl !Send for ScopedInterruptDisabler {}
impl !Sync for ScopedInterruptDisabler {}

impl ScopedInterruptDisabler {
    pub fn new() -> ScopedInterruptDisabler {
        let saved_flags = rflags::read_raw();

        unsafe {
            asm!("cli");
        }

        ScopedInterruptDisabler {
            saved_flags: saved_flags,
        }
    }
}

impl Drop for ScopedInterruptDisabler {
    fn drop(&mut self) {
        rflags::write_raw(self.saved_flags);
    }
}
