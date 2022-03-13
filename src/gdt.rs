/// Routines to set up the x86_64 GDT
///
/// The GDT in 64 bit mode has limited capabilities. It is important for
/// switching between userspace and kernel space, entering 32-bit compatibility
/// mode, and a couple other random things.
///
/// The code here only deals with the bare minimum GDT for running in ring-0,
/// 64-bit mode.
use x86_64::instructions::segmentation::*;
use x86_64::structures::gdt::*;
use x86_64::PrivilegeLevel;

use spin::mutex::{SpinMutex, SpinMutexGuard};

static GDT: SpinMutex<GlobalDescriptorTable> = SpinMutex::new(GlobalDescriptorTable::new());

pub fn init() {
    // Make sure we are only called once.
    static IS_INITIALIZED: core::sync::atomic::AtomicBool =
        core::sync::atomic::AtomicBool::new(false);
    assert!(!IS_INITIALIZED.swap(true, core::sync::atomic::Ordering::SeqCst));

    let gdt = SpinMutexGuard::leak(GDT.lock());
    gdt.add_entry(Descriptor::kernel_code_segment());
    // Not sure if this one is necessary?
    gdt.add_entry(Descriptor::kernel_data_segment());
    gdt.load();

    unsafe {
        CS::set_reg(SegmentSelector::new(1, PrivilegeLevel::Ring0));
        DS::set_reg(SegmentSelector::new(2, PrivilegeLevel::Ring0));
        ES::set_reg(SegmentSelector::new(2, PrivilegeLevel::Ring0));
        FS::set_reg(SegmentSelector::new(2, PrivilegeLevel::Ring0));
        GS::set_reg(SegmentSelector::new(2, PrivilegeLevel::Ring0));
        SS::set_reg(SegmentSelector::new(2, PrivilegeLevel::Ring0));
    }
}
