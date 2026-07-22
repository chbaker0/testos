#![no_main]
#![no_std]

extern crate alloc;

use shared::boot_info::BootInfo;
use shared::memory::page::{Frame, FrameRange, Page, PAGE_SIZE};
use shared::memory::paging::{Mapper, PageTable, PageTableFlags};
use shared::memory::{Length, Map, MapEntry, MemoryType as SharedMemoryType, PhysAddress, PhysExtent, VirtAddress};

use core::fmt::Write;
use core::writeln;

use log::info;

use uefi::mem::memory_map::{MemoryMap, MemoryMapMut};
use uefi::prelude::*;

use boot::{AllocateType, MemoryType};

#[entry]
fn main() -> Status {
    let image_handle = boot::image_handle();

    uefi::helpers::init().unwrap();

    let mut fs = boot::get_image_file_system(image_handle).expect("load fs protocol");
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

    let page_table_root_ptr =
        boot::allocate_pages(AllocateType::AnyPages, MemoryType::LOADER_DATA, 1)
            .unwrap()
            .as_ptr() as *mut PageTable;
    unsafe {
        page_table_root_ptr.write(PageTable::zero());
    }
    let mut page_mapper = unsafe {
        Mapper::new(
            &mut *page_table_root_ptr,
            |phys| Some(VirtAddress::from_raw(phys.as_raw())),
            || {
                Some(Frame::new(PhysAddress::from_raw(
                    boot::allocate_pages(AllocateType::AnyPages, MemoryType::LOADER_DATA, 1)
                        .ok()?
                        .as_ptr() as u64,
                )))
            },
        )
    };

    info!("Allocating pages for segments");

    for seg in kernel_elf.segments().expect("segment table").iter() {
        match seg.p_type {
            elf::abi::PT_LOAD => (),
            elf::abi::PT_DYNAMIC => panic!("found PT_DYNAMIC segment in kernel"),
            _ => continue,
        }

        let is_code = seg.p_flags & elf::abi::PF_X > 0;
        let is_writable = seg.p_flags & elf::abi::PF_W > 0;

        let page_count = Length::from_raw(seg.p_memsz).num_pages() as usize;
        let addr = PhysAddress::from_raw(
            boot::allocate_pages(
                AllocateType::AnyPages,
                if is_code {
                    MemoryType::LOADER_CODE
                } else {
                    MemoryType::LOADER_DATA
                },
                page_count,
            )
            .expect("allocating pages for kernel segment")
            .as_ptr() as u64,
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
        let parent_mask_flags = parent_set_flags;
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

    let mut init = dir
        .open(cstr16!("init"), FileMode::Read, FileAttribute::READ_ONLY)
        .expect("open init binary")
        .into_regular_file()
        .expect("regular file");

    let mut buf = [0; 1024];
    let file_info: &FileInfo = init.get_info(&mut buf).unwrap();
    let init_size = file_info.file_size() as usize;

    let init_page_count = Length::from_raw(init_size as u64).num_pages() as usize;
    let init_addr = PhysAddress::from_raw(
        boot::allocate_pages(AllocateType::AnyPages, MemoryType::LOADER_DATA, init_page_count)
            .expect("allocating pages for init module")
            .as_ptr() as u64,
    );

    // During UEFI boot, all memory is identity mapped.
    let init_slice =
        unsafe { core::slice::from_raw_parts_mut(init_addr.as_raw() as *mut u8, init_size) };
    init.read(init_slice).expect("reading init binary");

    let init_extent = PhysExtent::from_raw(init_addr.as_raw(), init_size as u64);

    info!("init module loaded");

    // Allocate the BootInfo page now, while boot services (and thus
    // allocate_pages) are still available. It's filled in below, after we
    // have the final memory map.
    let boot_info_ptr =
        boot::allocate_pages(AllocateType::AnyPages, MemoryType::LOADER_DATA, 1)
            .unwrap()
            .as_ptr() as *mut BootInfo;

    let mut mem_map = boot::memory_map(MemoryType::LOADER_DATA).expect("get mem map");
    mem_map.sort();

    let mut debugcon = unsafe { shared::log::QemuDebugWriter::new() };

    for e in mem_map.entries() {
        if !needs_identity_map(e.ty) {
            continue;
        }

        let frames = FrameRange::new(
            Frame::new(PhysAddress::from_raw(e.phys_start)),
            e.page_count as u64,
        )
        .unwrap();

        let parent_set_flags = PageTableFlags::DEFAULT_PARENT_TABLE_FLAGS;
        let parent_mask_flags = parent_set_flags;
        let leaf_flags = PageTableFlags::PRESENT | PageTableFlags::WRITABLE;

        for frame in frames.iter() {
            let page = Page::new(VirtAddress::from_raw(frame.start().as_raw()));
            unsafe {
                page_mapper
                    .map(page, frame, leaf_flags, parent_set_flags, parent_mask_flags)
                    .unwrap();
            }
        }
    }

    // Identity map the first 1 MiB unconditionally. Some of it (e.g. the
    // legacy VGA/compatibility hole at 0xA0000-0xFFFFF) isn't necessarily
    // covered by any UEFI memory map entry, but the kernel writes to VGA
    // memory (0xB8000) before it has mapped anything of its own.
    for frame in FrameRange::new(Frame::new(PhysAddress::from_raw(0)), 256)
        .unwrap()
        .iter()
    {
        let page = Page::new(VirtAddress::from_raw(frame.start().as_raw()));
        unsafe {
            page_mapper
                .map(
                    page,
                    frame,
                    PageTableFlags::PRESENT | PageTableFlags::WRITABLE,
                    PageTableFlags::DEFAULT_PARENT_TABLE_FLAGS,
                    PageTableFlags::DEFAULT_PARENT_TABLE_FLAGS,
                )
                .unwrap();
        }
    }

    let entry_addr: u64 = kernel_elf.ehdr.e_entry;
    info!("identity mapped existing memory. exiting boot services. kernel entry: {entry_addr:x}");

    let final_mem_map = unsafe { boot::exit_boot_services(None) };

    writeln!(debugcon, "Exited boot services").unwrap();

    let boot_info = BootInfo {
        memory_map: translate_memory_map(&final_mem_map),
        page_table_root: PhysAddress::from_raw(page_table_root_ptr as u64),
        init_module: init_extent,
    };
    unsafe {
        boot_info_ptr.write(boot_info);
    }

    unsafe {
        x86_64::registers::control::Cr3::write(
            x86_64::structures::paging::PhysFrame::from_start_address(x86_64::addr::PhysAddr::new(
                page_table_root_ptr as u64,
            ))
            .unwrap(),
            x86_64::registers::control::Cr3Flags::empty(),
        );
    }

    writeln!(debugcon, "Installed page table").unwrap();

    unsafe {
        core::arch::asm!(
            "jmp {entry_addr}",
            entry_addr = in(reg) entry_addr,
            in("rdi") boot_info_ptr as u64,
        );
    }

    unreachable!()
}

/// Whether a UEFI memory-map entry needs to be identity-mapped by the
/// loader. Skips types that are never RAM (`RESERVED`, `UNUSABLE`,
/// `MMIO_PORT_SPACE`, `PAL_CODE`) plus `MMIO`, which is only needed if the
/// OS calls UEFI runtime services after exiting boot services — this
/// kernel does not (grepped: no `runtime_services`/`SetVirtualAddressMap`
/// calls anywhere), so it's safe to skip too. Revisit if runtime services
/// support is ever added.
fn needs_identity_map(ty: MemoryType) -> bool {
    !matches!(
        ty,
        MemoryType::RESERVED
            | MemoryType::UNUSABLE
            | MemoryType::MMIO
            | MemoryType::MMIO_PORT_SPACE
            | MemoryType::PAL_CODE
    )
}

fn translate_memory_map(uefi_map: &impl MemoryMap) -> Map {
    use uefi::mem::memory_map::MemoryType as UefiType;

    Map::from_entries(uefi_map.entries().map(|area| MapEntry {
        extent: PhysExtent::from_raw(area.phys_start, area.page_count * PAGE_SIZE.as_raw()),
        mem_type: match area.ty {
            UefiType::CONVENTIONAL => SharedMemoryType::Available,
            UefiType::ACPI_RECLAIM => SharedMemoryType::Acpi,
            UefiType::LOADER_CODE | UefiType::LOADER_DATA => SharedMemoryType::KernelLoad,
            UefiType::UNUSABLE => SharedMemoryType::Defective,
            _ => SharedMemoryType::Reserved,
        },
    }))
}
