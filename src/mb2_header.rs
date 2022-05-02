use core::mem::size_of;

use multiboot2_header::*;

use HeaderTagFlag::*;

macro_rules! size_sum {
    () => (0);
    ($v:expr, $($rest:expr),*) => (core::mem::size_of_val($v) + size_sum!($($rest),*));
}

#[repr(C, packed)]
struct Header {
    magic: u32,
    architecture: HeaderTagISA,
    header_length: u32,
    checksum: u32,
    console_tag: ConsoleHeaderTag,

    mbi_request_tag_type: HeaderTagType,
    mbi_request_tag_flags: HeaderTagFlag,
    mbi_request_tag_size: u32,
    load_addr_request: MbiTagType,
    mmap_request: MbiTagType,
    mbi_request_end: MbiTagType,

    end_tag: EndHeaderTag,
}

const fn make_header() -> Header {
    let mut header = Header {
        magic: 0xE85250D6,
        architecture: HeaderTagISA::I386,
        header_length: size_of::<Header>() as u32,
        checksum: 0,
        console_tag: ConsoleHeaderTag::new(Required, ConsoleHeaderTagFlags::ConsoleRequired),
        mbi_request_tag_type: HeaderTagType::InformationRequest,
        mbi_request_tag_flags: HeaderTagFlag::Required,
        mbi_request_tag_size: 0,
    };

    header.checksum = 0 - (header.magic + header.architecture as u32 + header.header_length);
    header.mbi_request_tag_size = size_sum!(
        header.mbi_request_tag_type,
        header.mbi_request_tag_flags,
        header.mbi_request_tag_size,
        header.load_addr_request,
        header.mmap_request,
        header.mbi_request_end,
    ) as u32;
}

#[link_section = ".multiboot2"]
static HEADER: Header = make_header();
