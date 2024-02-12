//! Basic logging facilities used with the `log` crate.

use core::fmt::Write;
use core::marker::Send;

use log::{Level, Log, Metadata, Record};
use spin::Mutex;

/// Extended `Log` interface for OS.
pub trait LogExt {
    /// Check if the logger impl is locked. For example, if a logging operation
    /// itself caused a panic, it can be left in a locked (and invalid) state. A
    /// panic handler may check this and use a backup method if so.
    fn is_locked(&self) -> bool;
}

/// Writes formatted log messages to any `core::fmt::Write` impl. Locks
/// internally.
pub struct LogSink<W> {
    writer: Mutex<W>,
}

impl<W: Write + Send> LogSink<W> {
    pub fn new(writer: W) -> Self {
        LogSink {
            writer: Mutex::new(writer),
        }
    }
}

impl<W: Write + Send> Log for LogSink<W> {
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

impl<W: Write + Send> LogExt for LogSink<W> {
    fn is_locked(&self) -> bool {
        self.writer.is_locked()
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

/// Forwards the same message to two loggers. The loggers are called in order
/// every time.
pub struct LogTee<L1, L2>(pub L1, pub L2);

impl<L1: Log, L2: Log> Log for LogTee<L1, L2> {
    fn enabled(&self, metadata: &Metadata) -> bool {
        self.0.enabled(metadata) || self.1.enabled(metadata)
    }

    fn log(&self, record: &Record) {
        self.0.log(record);
        self.1.log(record);
    }

    fn flush(&self) {
        self.0.flush();
        self.1.flush();
    }
}

impl<L1: LogExt, L2: LogExt> LogExt for LogTee<L1, L2> {
    fn is_locked(&self) -> bool {
        self.0.is_locked() || self.1.is_locked()
    }
}

/// Writes to QEMU's debug out port.
pub struct QemuDebugWriter {
    _phantom: core::marker::PhantomData<*mut u8>,
}

unsafe impl Send for QemuDebugWriter {}

impl QemuDebugWriter {
    /// # Safety
    ///
    /// Caller must ensure x86 port 0xe9 is safe to write to.
    pub unsafe fn new() -> Self {
        QemuDebugWriter {
            _phantom: core::marker::PhantomData,
        }
    }
}

impl Write for QemuDebugWriter {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        let mut port = x86_64::instructions::port::PortWriteOnly::new(0xe9);
        s.bytes().for_each(|b| unsafe { port.write(b) });
        Ok(())
    }
}
