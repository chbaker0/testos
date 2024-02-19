#![no_main]
#![no_std]

extern crate alloc;

use log::info;
use uefi::prelude::*;

#[entry]
fn main(image_handle: Handle, mut system_table: SystemTable<Boot>) -> Status {
    uefi_services::init(&mut system_table).unwrap();
    let mut bs = system_table.boot_services();

    let mut fs = bs
        .get_image_file_system(image_handle)
        .expect("load fs protocol");
    let mut dir = fs.open_volume().expect("open fs");

    use uefi::proto::media::file::{File, FileAttribute, FileInfo, FileMode};
    let mut kernel = dir
        .open(cstr16!("testos"), FileMode::Read, FileAttribute::READ_ONLY)
        .expect("open testos binary")
        .into_regular_file()
        .expect("regular file");

    info!("Opened testos binary");

    let mut buf = [0; 1024];
    let file_info: &FileInfo = kernel.get_info(&mut buf).unwrap();
    let mut kernel_image = alloc::vec::Vec::new();
    kernel_image.resize(file_info.file_size() as usize, 0);
    kernel
        .read(&mut kernel_image)
        .expect("reading testos binary");

    let kernel_elf = elf::ElfBytes::minimal_parse(&kernel_image).expect("parsing elf");

    info!("Allocating pages for segments");

    for seg in kernel_elf.segments().expect("segment table").iter() {
        use uefi::table::boot::{AllocateType, MemoryType};

        let page_count = shared::memory::Length::from_raw(seg.memsz).num_pages() as usize;
        bs.allocate_pages(AllocateType::AnyPages, MemoryType, page_count);
    }

    loop {
        x86_64::instructions::hlt()
    }
}
