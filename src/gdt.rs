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

// This is unavoidably static mut. GlobalDescriptorTable::load takes a static
// reference to self. Wrapping this in a mutex means the mutex would need to
// stay locked forever.
static mut GDT: GlobalDescriptorTable = GlobalDescriptorTable::new();

pub fn init() {
    // Make sure we are only called once.
    static IS_INITIALIZED: core::sync::atomic::AtomicBool =
        core::sync::atomic::AtomicBool::new(false);
    assert!(!IS_INITIALIZED.swap(true, core::sync::atomic::Ordering::SeqCst));

    unsafe {
        GDT = GlobalDescriptorTable::new();

        GDT.add_entry(Descriptor::kernel_code_segment());
        // Not sure if this one is necessary?
        GDT.add_entry(Descriptor::kernel_data_segment());
        GDT.load();

        set_cs(SegmentSelector::new(1, PrivilegeLevel::Ring0));
        load_ds(SegmentSelector::new(2, PrivilegeLevel::Ring0));
        load_es(SegmentSelector::new(2, PrivilegeLevel::Ring0));
        load_fs(SegmentSelector::new(2, PrivilegeLevel::Ring0));
        load_gs(SegmentSelector::new(2, PrivilegeLevel::Ring0));
        load_ss(SegmentSelector::new(2, PrivilegeLevel::Ring0));
    }
}
