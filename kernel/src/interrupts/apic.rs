use core::ptr::{read_volatile, write_volatile};
use core::sync::atomic;
use mm;
use x86_64::registers::msr;

const LOCAL_APIC_PHYSICAL_BASE: u64 = 0xfee00000;
static LOCAL_APIC_ADDR: atomic::AtomicU64 = atomic::ATOMIC_U64_INIT;

const LOCAL_APIC_REG_SPURIOUS_INTERRUPT: u64 = 0xf0;

fn read_local_apic_reg(reg: u64) -> u32 {
    unsafe {
        read_volatile(LOCAL_APIC_ADDR.load(atomic::Ordering::SeqCst) as *const u32)
    }
}

fn write_local_apic_reg(reg: u64, val: u32) {
    unsafe {
        write_volatile(LOCAL_APIC_ADDR.load(atomic::Ordering::SeqCst) as *mut u32, val)
    }
}

pub fn init() {
    // Disable PIC.
    super::pic::disable();

    // Enable local APIC.
    unsafe {
        msr::wrmsr(msr::IA32_APIC_BASE, LOCAL_APIC_PHYSICAL_BASE | 0x800);
    }

    // Map local APIC registers into virtual memory.
    let page = mm::allocate_address_space(1).unwrap();
    mm::map_to(mm::Page(page), mm::Frame(LOCAL_APIC_PHYSICAL_BASE >> 12), mm::paging::PAGE_FLAG_CACHE_DISABLE | mm::paging::PAGE_FLAG_WRITE_THROUGH, mm::get_frame_allocator());
    LOCAL_APIC_ADDR.store(page << 12, atomic::Ordering::SeqCst);

    // Enable interrupts.
    write_local_apic_reg(LOCAL_APIC_REG_SPURIOUS_INTERRUPT, 0x1ff);
}
