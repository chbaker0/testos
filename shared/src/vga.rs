//! VGA helpers

use core::fmt::Write;

const ROWS: usize = 25;
const COLS: usize = 80;

pub struct VgaWriter {
    vmem: *mut u8,
    offset: usize,
}

impl VgaWriter {
    pub unsafe fn new(vmem: *mut u8) -> VgaWriter {
        let mut vga_writer = VgaWriter { vmem, offset: 0 };
        vga_writer.clear();
        vga_writer
    }

    pub fn clear(&mut self) {
        for i in 0..ROWS * COLS {
            unsafe {
                *self.vmem.offset(2 * i as isize) = 0;
            }
        }

        self.offset = 0;
    }
}

impl Write for VgaWriter {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        for c in s.chars() {
            if self.offset >= ROWS * COLS {
                return Err(core::fmt::Error);
            }

            if c == '\n' {
                self.offset = ((self.offset + COLS) / COLS) * COLS;
                continue;
            }

            let b = if c.is_ascii() { c as u8 } else { '?' as u8 };

            unsafe {
                *self.vmem.offset(2 * self.offset as isize) = b;
            }

            self.offset += 1;
        }

        Ok(())
    }
}
