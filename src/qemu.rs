//! Minimal QEMU control via the `isa-debug-exit` device.
//!
//! When QEMU is launched with `-device isa-debug-exit,iobase=0xf4,iosize=0x04`
//! (as CI's smoke test does), writing a `u32` `v` to port `0xf4` makes the
//! QEMU process exit with status `(v << 1) | 1`. When that device is *not*
//! attached — i.e. every normal interactive run — the write lands on an
//! unassigned I/O port and QEMU silently ignores it, so [`exit_qemu`] is a
//! harmless no-op outside CI and needs no feature gate.

/// Values written to the `isa-debug-exit` port.
///
/// The resulting process exit codes are `(value << 1) | 1`, so `Success`
/// yields `33` and `Failed` yields `35`. Note `0` is unreachable through this
/// device, so CI keys success off the specific `33` rather than the usual
/// "exit code 0".
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum QemuExitCode {
    Success = 0x10,
    Failed = 0x11,
}

/// Ask QEMU to exit with a status derived from `exit_code`.
///
/// No-op when the `isa-debug-exit` device is not attached (normal runs).
pub fn exit_qemu(exit_code: QemuExitCode) {
    use x86_64::instructions::port::PortWriteOnly;
    // SAFETY: 0xf4 is the conventional `isa-debug-exit` iobase and is not used
    // for anything else in this kernel. Writing to it is either the intended
    // exit signal (device present) or ignored by QEMU (device absent).
    unsafe {
        let mut port = PortWriteOnly::new(0xf4);
        port.write(exit_code as u32);
    }
}
