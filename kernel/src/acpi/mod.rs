pub mod osl;

use ::write_terminal;
use core::ptr::null_mut;
use core::slice;

type AcpiStatus = u32;

#[repr(C, packed)]
struct AcpiTableHeader {
    signature: [u8; 4],
    length: u32,
    revision: u8,
    checksum: u8,
    oem_id: [u8; 6],
    oem_table_id: [u8; 8],
    oem_revision: u32,
    creator_id: u32,
    creator_revision: u32,
}

#[repr(C, packed)]
struct AcpiTableDesc {
    physical_address: u64,
    header: *mut AcpiTableHeader,
    length: u32,
    signature: u32,
    owner_id: u8,
    flags: u8,
    validation_count: u16,
}

extern "C" {
    fn AcpiInitializeTables(initial_table_array: *mut AcpiTableDesc,
                            initial_table_count: u32,
                            allow_resize: bool);
}

#[repr(C, packed)]
struct RSDP {
    signature: [u8; 8],
    checksum: u8,
    oemid: [u8; 6],
    revision: u8,
    rsdt_addr: u32,
    length: u32,
    xsdt_addr: u64,
    extended_checksum: u8,
    reserved: [u8; 3],
}

unsafe fn check_rsdp(ptr: *const u8) -> bool {
    let sig = slice::from_raw_parts(ptr, 8);
    if sig == "RSD PTR ".as_bytes() {
        true
    } else {
        false
    }
}

fn find_rsdp() -> *const RSDP {
    for i in (0x00080000..0x00080400).step_by(16) {
        if unsafe { check_rsdp(i as *const u8) } {
            return i as *const RSDP;
        }
    }

    for i in (0x000E0000..0x00100000).step_by(16) {
        if unsafe { check_rsdp(i as *const u8) } {
            return i as *const RSDP;
        }
    }

    panic!("RSDP not found.");
}

pub fn init() {
    let rsdpp = find_rsdp();
    write_terminal(format_args!("RSDP found at {:x}.", rsdpp as usize));
    unsafe {
        AcpiInitializeTables(null_mut(), 0, false);
    }
}
