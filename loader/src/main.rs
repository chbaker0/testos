#![no_main]
#![no_std]

extern crate alloc;

use shared::memory::page::{Frame, FrameRange, Page};
use shared::memory::paging::{Mapper, PageTable, PageTableFlags};
use shared::memory::{Length, PhysAddress, VirtAddress};

use log::info;
use uefi::prelude::*;
use uefi::table::boot::{AllocateType, MemoryType};

#[entry]
fn main(image_handle: Handle, mut system_table: SystemTable<Boot>) -> Status {
    uefi_services::init(&mut system_table).unwrap();

    let mut fs = system_table
        .boot_services()
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

    core::mem::drop(fs);

    let kernel_elf = elf::ElfBytes::<'_, elf::endian::LittleEndian>::minimal_parse(&kernel_image)
        .expect("parsing elf");

    let page_table_root_ptr = system_table
        .boot_services()
        .allocate_pages(AllocateType::AnyPages, MemoryType::LOADER_DATA, 1)
        .unwrap() as *mut PageTable;
    unsafe {
        page_table_root_ptr.write(PageTable::zero());
    }
    let mut page_mapper = unsafe {
        Mapper::new(
            &mut *page_table_root_ptr,
            |phys| Some(VirtAddress::from_raw(phys.as_raw())),
            || {
                Some(Frame::new(PhysAddress::from_raw(
                    system_table
                        .boot_services()
                        .allocate_pages(AllocateType::AnyPages, MemoryType::LOADER_DATA, 1)
                        .ok()?,
                )))
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

        let is_code = seg.p_flags & elf::abi::PF_X > 0;
        let is_writable = seg.p_flags & elf::abi::PF_W > 0;

        let page_count = Length::from_raw(seg.p_memsz).num_pages() as usize;
        let addr = PhysAddress::from_raw(
            system_table
                .boot_services()
                .allocate_pages(
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

    let mut mem_map_buf = [0; 4096 * 16];
    let mem_map = system_table
        .boot_services()
        .memory_map(&mut mem_map_buf)
        .expect("get mem map");

    for e in mem_map.entries() {
        let frames = FrameRange::new(
            Frame::new(PhysAddress::from_raw(e.phys_start)),
            e.page_count as u64,
        )
        .unwrap();
        let first_page = Page::new(VirtAddress::from_raw(e.virt_start));

        let parent_set_flags = PageTableFlags::DEFAULT_PARENT_TABLE_FLAGS;
        let parent_mask_flags = PageTableFlags::empty();
        let leaf_flags = PageTableFlags::PRESENT
            | PageTableFlags::WRITABLE
            | match e.ty {
                MemoryType::LOADER_CODE
                | MemoryType::BOOT_SERVICES_CODE
                | MemoryType::RUNTIME_SERVICES_CODE => PageTableFlags::empty(),
                _ => PageTableFlags::EXECUTE_DISABLE,
            };

        for (n, frame) in frames.iter().enumerate() {
            let page = first_page.next(n as u64).unwrap();
            unsafe {
                page_mapper
                    .map(page, frame, leaf_flags, parent_set_flags, parent_mask_flags)
                    .unwrap();
            }
        }
    }

    let entry_addr: u64 = kernel_elf.ehdr.e_entry;
    info!("identity mapped existing memory. exiting boot services. kernel entry: {entry_addr:x}");
    system_table.boot_services().stall(5 * 1000 * 1000);

    let (_system_table, _mem_map) = system_table.exit_boot_services(MemoryType::LOADER_DATA);

    unsafe {
        x86_64::registers::control::Cr3::write(
            x86_64::structures::paging::PhysFrame::from_start_address(x86_64::addr::PhysAddr::new(
                page_table_root_ptr as u64,
            ))
            .unwrap(),
            x86_64::registers::control::Cr3Flags::empty(),
        );

        core::arch::asm!(
            "jmp {entry_addr}",
            entry_addr = in(reg) entry_addr,
        );
    }

    unreachable!()
}
