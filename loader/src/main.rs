#![no_main]
#![no_std]

extern crate alloc;

use log::info;
use uefi::prelude::*;
use uefi::table::boot::{AllocateType, MemoryType};

#[entry]
fn main(image_handle: Handle, mut system_table: SystemTable<Boot>) -> Status {
    uefi_services::init(&mut system_table).unwrap();
    let bs = system_table.boot_services();

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

    let kernel_elf = elf::ElfBytes::<'_, elf::endian::LittleEndian>::minimal_parse(&kernel_image)
        .expect("parsing elf");

    let page_table_root_ptr = bs
        .allocate_pages(AllocateType::AnyPages, MemoryType::LOADER_DATA, 1)
        .unwrap() as *mut shared::memory::paging::PageTable;
    unsafe {
        page_table_root_ptr.write(shared::memory::paging::PageTable::zero());
    }
    let mut page_mapper = unsafe {
        shared::memory::paging::Mapper::new(
            &mut *page_table_root_ptr,
            |phys| Some(shared::memory::VirtAddress::from_raw(phys.as_raw())),
            || {
                Some(shared::memory::page::Frame::new(
                    shared::memory::PhysAddress::from_raw(
                        bs.allocate_pages(AllocateType::AnyPages, MemoryType::LOADER_DATA, 1)
                            .ok()?,
                    ),
                ))
            },
        )
    };

    info!("Allocating pages for segments");

    for seg in kernel_elf.segments().expect("segment table").iter() {
        use shared::memory::{page::PAGE_SIZE, PhysAddress};

        match seg.p_type {
            elf::abi::PT_LOAD => (),
            elf::abi::PT_DYNAMIC => panic!("found PT_DYNAMIC segment in kernel"),
            _ => continue,
        }

        let is_code = seg.p_flags | elf::abi::PF_X > 0;
        let is_writable = seg.p_flags | elf::abi::PF_W > 0;

        let page_count = shared::memory::Length::from_raw(seg.p_memsz).num_pages() as usize;
        let addr = PhysAddress::from_raw(
            bs.allocate_pages(
                AllocateType::AnyPages,
                if is_code {
                    MemoryType::LOADER_CODE
                } else {
                    MemoryType::LOADER_DATA
                },
                page_count,
            )
            .expect("allocating pages for kernel segment"),
        );

        // During UEFI boot, all memory is identity mapped.
        let to_ptr = addr.as_raw() as *mut u8;
        unsafe {
            to_ptr.write_bytes(0, page_count * PAGE_SIZE.as_raw() as usize);
        }

        let image_data = kernel_elf.segment_data(&seg).expect("reading segment data");
        assert!(image_data.len() as u64 <= seg.p_memsz);
        unsafe {
            to_ptr.copy_from_nonoverlapping(image_data.as_ptr(), image_data.len());
        }

        use shared::memory::page::{Frame, FrameRange, Page};
        use shared::memory::paging::PageTableFlags;
        use shared::memory::VirtAddress;
        let parent_set_flags = PageTableFlags::DEFAULT_PARENT_TABLE_FLAGS;
        let parent_mask_flags = PageTableFlags::empty();
        let mut leaf_flags = PageTableFlags::PRESENT;
        if !is_code {
            leaf_flags |= PageTableFlags::EXECUTE_DISABLE;
        }
        if is_writable {
            leaf_flags |= PageTableFlags::WRITABLE;
        }

        let first_page = Page::new(VirtAddress::from_raw(seg.p_vaddr));
        for (n, frame) in FrameRange::new(Frame::new(addr), page_count as u64)
            .unwrap()
            .iter()
            .enumerate()
        {
            let page = first_page.next(n as u64).unwrap();
            unsafe {
                page_mapper
                    .map(page, frame, leaf_flags, parent_set_flags, parent_mask_flags)
                    .unwrap();
            }
        }
    }

    info!("kernel loaded and mapped");

    loop {
        x86_64::instructions::hlt()
    }
}
