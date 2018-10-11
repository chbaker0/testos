use ::terminal;
use ::vga;

use core::default::Default;
use core::fmt::*;
use core::ops::DerefMut;
use log::Log;
use spin::Mutex;

static VGA_LOG: VgaLog = VgaLog { buffer: spin::Mutex::new(terminal::Buffer::new()) };

pub fn init() {
    log::set_logger(&VGA_LOG).unwrap();
    log::set_max_level(log::LevelFilter::Debug);
}

struct VgaLog {
    buffer: spin::Mutex<terminal::Buffer>,
}

impl Log for VgaLog {
    fn enabled(&self, _metadata: &log::Metadata) -> bool {
        true
    }

    fn log(&self, record: &log::Record) {
        if self.enabled(record.metadata()) {
            let mut buffer_writer = BufferWriter {
                buffer: [0; 1024],
                ndx: 0,
            };
            write(&mut buffer_writer, *record.args()).unwrap();

            let mut buffer = self.buffer.lock();
            for (i, c) in buffer_writer.buffer.iter().take(terminal::WIDTH).enumerate() {
                let bottom_line = buffer.bottom_line;
                buffer.data[bottom_line].0[i] = *c;
            }
            buffer.bottom_line += 1;

            let mut term = vga::VGA_TERMINAL.lock();
            terminal::display_buffer(term.deref_mut(), &buffer);
        }
    }

    fn flush(&self) {
        // Do nothing
    }
}

struct BufferWriter {
    buffer: [u8; 1024],
    ndx: usize,
}

impl Write for BufferWriter {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        let trunc = s.bytes().take(self.buffer.len()-self.ndx);
        for c in trunc {
            self.buffer[self.ndx] = c;
            self.ndx += 1;
        }
        Ok(())
    }
}
