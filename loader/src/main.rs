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
    // SAFETY: `boot::allocate_pages` just returned this page fresh from UEFI
    // boot services, so it's valid, page-aligned (>= `PageTable`'s required
    // 4096-byte alignment), and not referenced anywhere else yet.
    unsafe {
        page_table_root_ptr.write(PageTable::zero());
    }
    // SAFETY: `page_table_root_ptr` was just written with a valid, zeroed L4
    // table and isn't referenced anywhere else (satisfying `Mapper::new`'s
    // `level_4` requirement; it's also not yet the active page table, so the
    // "if active, don't break live translations" clause doesn't apply). The
    // translator is the identity function, which UEFI guarantees is valid
    // for all memory while boot services are active (we haven't called
    // `exit_boot_services` yet). The frame allocator calls
    // `boot::allocate_pages` fresh each time, and UEFI guarantees it never
    // hands out overlapping allocations, so frames are never in use
    // elsewhere.
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

    // Track the physical span the loader places the kernel's segments into, so
    // the kernel learns where it actually lives (its linker symbols only
    // describe virtual addresses). See `BootInfo::kernel_image`.
    let mut kernel_phys_begin: Option<u64> = None;
    let mut kernel_phys_end: u64 = 0;

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

        let seg_begin = addr.as_raw();
        let seg_end = seg_begin + page_count as u64 * PAGE_SIZE.as_raw();
        kernel_phys_begin = Some(kernel_phys_begin.map_or(seg_begin, |b| b.min(seg_begin)));
        kernel_phys_end = kernel_phys_end.max(seg_end);

        // During UEFI boot, all memory is identity mapped.
        let to_ptr = addr.as_raw() as *mut u8;
        // SAFETY: `addr` is `page_count` pages just handed out fresh by
        // `boot::allocate_pages` above (not referenced anywhere else, and
        // UEFI guarantees no other allocation overlaps it), and `to_ptr` is
        // its identity-mapped address, valid to dereference while boot
        // services are still active. `page_count * PAGE_SIZE` is exactly the
        // size just allocated.
        unsafe {
            to_ptr.write_bytes(0, page_count * PAGE_SIZE.as_raw() as usize);
        }

        let image_data = kernel_elf.segment_data(&seg).expect("reading segment data");
        assert!(image_data.len() as u64 <= seg.p_memsz);
        // SAFETY: `to_ptr` is valid for `page_count * PAGE_SIZE.as_raw()`
        // bytes as above, and `image_data.len() <= seg.p_memsz` (asserted)
        // fits within that (`page_count` is computed from `seg.p_memsz`).
        // `image_data` borrows `kernel_image`, a separate `Vec` allocation,
        // so it cannot overlap `to_ptr`'s freshly UEFI-allocated pages.
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
            // SAFETY: `page_mapper`'s table isn't active yet (CR3 switches
            // only at the bottom of `main`), so no live translation is broken.
            // PT_LOAD segments are byte-disjoint but may still share a page in
            // general; here every section gets `ALIGN(4K)` from
            // `src/linker.ld`, so the segments' page ranges are disjoint too
            // and no `page` is mapped twice.
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
    // SAFETY: `init_addr` is `init_page_count` pages just handed out fresh
    // by `boot::allocate_pages` (not referenced anywhere else, and UEFI
    // guarantees no other allocation overlaps it), identity-mapped and valid
    // to dereference while boot services are active. `init_size` fits
    // within `init_page_count` pages by construction (`num_pages()` rounds
    // up), so the slice stays within the allocation.
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

    // SAFETY: port 0xe9 is the conventional QEMU debugcon port; nothing else
    // in the loader writes to it, and no other `QemuDebugWriter` exists
    // concurrently (single-threaded UEFI application).
    let mut debugcon = unsafe { shared::log::QemuDebugWriter::new() };

    for e in mem_map.entries() {
        // Skip address-space holes (MMIO and other reserved ranges) and
        // unusable RAM. On real firmware these span multi-GiB regions; mapping
        // them at 4 KiB granularity dominates boot time (issue #5) and the
        // kernel never touches them during early boot. Boot-services memory is
        // deliberately kept — the kernel keeps running on the stack it
        // inherited from the loader (which lives there) after the CR3 switch.
        if !should_identity_map(e.ty) {
            continue;
        }

        let extent = PhysExtent::from_raw(e.phys_start, e.page_count * PAGE_SIZE.as_raw());

        let parent_set_flags = PageTableFlags::DEFAULT_PARENT_TABLE_FLAGS;
        let parent_mask_flags = parent_set_flags;
        let leaf_flags = PageTableFlags::PRESENT | PageTableFlags::WRITABLE;

        // Identity map: the virtual base numerically equals the entry's
        // physical start. Greedily uses 1 GiB/2 MiB pages where the extent
        // is large and aligned enough, instead of one 4 KiB `map` call per
        // frame — the fix for issue #5's boot-time symptom for whichever
        // entries still need mapping after the type filter above.
        // SAFETY: `page_mapper`'s table isn't active yet (CR3 switches only
        // after this loop). `mem_map` entries are disjoint (it's the UEFI-
        // reported memory map), so each iteration maps a fresh region;
        // `VirtAddress::from_raw(e.phys_start)` numerically equals
        // `extent.address()`, so the identity-map offset (zero) is aligned
        // to any page size `map_range` might pick.
        unsafe {
            page_mapper
                .map_range(
                    extent,
                    VirtAddress::from_raw(e.phys_start),
                    leaf_flags,
                    parent_set_flags,
                    parent_mask_flags,
                )
                .unwrap();
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
        // TODO(chbaker0): if the memory-map loop above already mapped an
        // address in this range using a huge page (`map_large::<Size1G>` or
        // `<Size2M>` inside `map_range`, when a `should_identity_map`-passing
        // entry starting at/before this range happens to be large and
        // aligned enough), this `map()` call would re-descend through that
        // entry's L3/L2 slot as if it were a parent-table pointer, even
        // though it's actually a huge-page leaf (`PRESENT` with `PAGE_SIZE`
        // set) — `next_level`'s `debug_assert!` would catch this in a debug
        // build, but a release build would silently misinterpret the leaf's
        // packed address bits as a child table frame. I couldn't establish
        // whether any current memory map (real firmware or the QEMU/OVMF
        // config this project boots) ever produces a low-memory entry both
        // `Available`/`KernelLoad`-like and large+aligned enough to hit this
        // (`map_range`'s phase 3 needs `>= 1 GiB` aligned remaining), so I'm
        // flagging it rather than asserting either that it's fine or that
        // it's a live bug.
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

    // SAFETY: everything the loader still needs past this point —
    // `kernel_image`/`init_slice` contents (already copied out of files),
    // `page_table_root_ptr`/`page_mapper` (built with `boot::allocate_pages`
    // frames, not boot-services-owned buffers), `boot_info_ptr` (likewise) —
    // was obtained before this call and doesn't depend on boot services
    // remaining available; no boot-services API (`fs`, `dir`,
    // `boot::allocate_pages`, etc.) is used again after this line.
    let final_mem_map = unsafe { boot::exit_boot_services(None) };

    writeln!(debugcon, "Exited boot services").unwrap();

    let boot_info = BootInfo {
        memory_map: translate_memory_map(&final_mem_map),
        page_table_root: PhysAddress::from_raw(page_table_root_ptr as u64),
        init_module: init_extent,
        kernel_image: {
            let begin = kernel_phys_begin.expect("kernel has at least one PT_LOAD segment");
            PhysExtent::from_raw(begin, kernel_phys_end - begin)
        },
    };
    // SAFETY: `boot_info_ptr` is a page allocated (and reserved from further
    // allocation) via `boot::allocate_pages` earlier and not referenced
    // anywhere else; `BootInfo` is small enough to fit in one page. This is
    // the only write to it, establishing the contract `kernel_entry` (see
    // src/kmain.rs) relies on when it dereferences the pointer handed to it
    // in `rdi` below.
    unsafe {
        boot_info_ptr.write(boot_info);
    }

    // SAFETY: `page_table_root_ptr` was fully built by `page_mapper` above —
    // it maps the kernel image (just-loaded segments) and identity-maps
    // physical memory (the memory-map loop and the first-1-MiB loop) — so
    // switching CR3 to it doesn't break any translation the loader's own
    // currently-executing code and stack (which lives in identity-mapped
    // memory) depend on.
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

    // SAFETY: `entry_addr` (`kernel_elf.ehdr.e_entry`) is the kernel ELF's
    // documented entry point, which is `kernel_entry` in src/kmain.rs —
    // `#[unsafe(export_name = "_start")]`, `extern "C" fn(*const BootInfo)`.
    // The just-installed page table (`Cr3::write` above) maps that virtual
    // address (it maps the whole kernel image), and `rdi` carries
    // `boot_info_ptr`'s physical address, matching `kernel_entry`'s sole
    // argument per the C calling convention — establishing the contract its
    // own `# Safety`/`SAFETY` comments (in src/kmain.rs) rely on: that the
    // pointer it's handed is valid and identity-mapped.
    unsafe {
        core::arch::asm!(
            "jmp {entry_addr}",
            entry_addr = in(reg) entry_addr,
            in("rdi") boot_info_ptr as u64,
        );
    }

    unreachable!()
}

/// Whether the loader should identity-map a UEFI memory range into the
/// kernel's transitional address space. Skips address-space holes (MMIO and
/// other reserved ranges) and unusable/unaccepted RAM; keeps everything else,
/// including boot-services memory (see the call site for why the stack lives
/// there). See issue #5.
fn should_identity_map(ty: uefi::mem::memory_map::MemoryType) -> bool {
    use uefi::mem::memory_map::MemoryType as UefiType;
    ty != UefiType::RESERVED
        && ty != UefiType::UNUSABLE
        && ty != UefiType::MMIO
        && ty != UefiType::MMIO_PORT_SPACE
        && ty != UefiType::PAL_CODE
        && ty != UefiType::UNACCEPTED
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
