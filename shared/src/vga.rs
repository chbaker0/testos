//! VGA helpers

use core::fmt::Write;

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
    /// * `vmem` must point to VGA text memory of at least `ROWS * COLS * 2`
    ///   bytes, valid for reads and writes for this writer's lifetime
    /// * only one instance must exist at a time; nothing here enforces it
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
            // SAFETY: `line < ROWS` (asserted) and `i < COLS`, so
            // `2 * (i + line * COLS)` is within the `ROWS * COLS * 2` bytes
            // `VgaWriter::new`'s contract guarantees.
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

        // SAFETY: `lines < ROWS` here (the `== ROWS` case returned above), so
        // both the source at `lines * COLS * 2` and the `(ROWS - lines) * COLS
        // * 2` bytes from `self.vmem` lie within the `ROWS * COLS * 2` bytes
        // `VgaWriter::new`'s contract guarantees. `copy`, not
        // `copy_nonoverlapping`, because the ranges overlap when scrolling.
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

// SAFETY: `VgaWriter` touches memory only through its own `vmem` pointer,
// never global or thread-local state, so moving one across threads
// invalidates nothing. Exclusive access to the VGA buffer comes from `new`'s
// one-instance requirement, not from this impl; there is no `Sync` impl, so
// `Send` alone doesn't permit sharing.
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

            // SAFETY: the scroll/bounds check above guarantees
            // `self.offset < ROWS * COLS`, so `2 * self.offset` is within the
            // `ROWS * COLS * 2` bytes `VgaWriter::new`'s contract guarantees.
            unsafe {
                *self.vmem.offset(2 * self.offset as isize) = b;
            }

            self.offset += 1;
        }

        Ok(())
    }
}

pub type VgaLog = crate::log::LogSink<VgaWriter>;
