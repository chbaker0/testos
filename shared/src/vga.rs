//! VGA helpers

use core::fmt::Write;
use log::*;
use spin::Mutex;

const ROWS: usize = 25;
const COLS: usize = 80;

pub struct VgaWriter {
    vmem: *mut u8,
    offset: usize,
}

impl VgaWriter {
    /// Create formatter writing to raw vga memory at `vmem`.
    ///
    /// # Safety
    /// * `vmem` must point to valid VGA memory
    /// * only one instance should exist
    pub unsafe fn new(vmem: *mut u8) -> VgaWriter {
        let mut vga_writer = VgaWriter { vmem, offset: 0 };
        vga_writer.clear();
        vga_writer
    }

    pub fn clear(&mut self) {
        for i in 0..ROWS {
            self.clear_line(i);
        }

        self.offset = 0;
    }

    fn clear_line(&mut self, line: usize) {
        assert!(line < ROWS);
        for i in 0..COLS {
            unsafe {
                *self.vmem.offset(2 * (i + line * COLS) as isize) = 0;
            }
        }
    }

    fn scroll(&mut self, lines: usize) {
        if lines == 0 {
            return;
        }

        let lines = core::cmp::min(lines, ROWS);
        if lines == ROWS {
            self.clear();
            return;
        }

        unsafe {
            core::ptr::copy(
                self.vmem.add(lines * COLS * 2),
                self.vmem,
                (ROWS - lines) * COLS * 2,
            );
        }

        for i in (ROWS - lines)..ROWS {
            self.clear_line(i);
        }

        self.offset = self.offset.saturating_sub(lines * COLS);
    }
}

unsafe impl Send for VgaWriter {}

impl Write for VgaWriter {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        for c in s.chars() {
            if self.offset >= ROWS * COLS {
                self.scroll(1);
                assert!(self.offset < ROWS * COLS);
            }

            if c == '\n' {
                self.offset = ((self.offset + COLS) / COLS) * COLS;
                continue;
            }

            let b = if c.is_ascii() { c as u8 } else { b'?' };

            unsafe {
                *self.vmem.offset(2 * self.offset as isize) = b;
            }

            self.offset += 1;
        }

        Ok(())
    }
}

pub struct VgaLog {
    writer: Mutex<VgaWriter>,
}

impl VgaLog {
    pub fn new(writer: VgaWriter) -> VgaLog {
        VgaLog {
            writer: Mutex::new(writer),
        }
    }
}

impl Log for VgaLog {
    fn enabled(&self, _: &Metadata) -> bool {
        true
    }

    fn log(&self, record: &Record) {
        let mut writer = self.writer.lock();
        let _ = writeln!(
            &mut writer,
            "[{}] {}: {}",
            level_as_string(record.level()),
            record.target(),
            record.args()
        );
    }

    fn flush(&self) {
        // No-op since we write directly to screen.
    }
}

fn level_as_string(level: Level) -> &'static str {
    use Level::*;

    match level {
        Error => "ERROR",
        Warn => " WARN",
        Info => " INFO",
        Debug => "DEBUG",
        Trace => "TRACE",
    }
}
