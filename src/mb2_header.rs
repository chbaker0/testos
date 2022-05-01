use core::mem::size_of;

use multiboot2_header::*;

use HeaderTagFlag::*;

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

#[link_section = ".multiboot2"]
static HEADER: Header = Header {
    console_tag: ConsoleHeaderTag::new(Required, ConsoleHeaderTagFlags::ConsoleRequired),
};
