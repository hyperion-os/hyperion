//! [`PageMap`] is the Page Table Manager
//!
// //! pages are mapped lazily when accessed according to the following mapping table
//!
//! | virtual                 | physical     | ~size                        | notes                               |
//! |-------------------------|--------------|------------------------------|-------------------------------------|
//! | `0x0`                   | -            | `0x1000` (1KiB)              | Null ptr guard page                 |
//! | `0x1000`                | ? (dynamic)  | TODO                         | User executable                     |
//! | TODO                    | ? (dynamic)  | TODO                         | User heap                           |
//! | `0x7FFB_FFFF_F000` [^1] | ? (dynamic)  | `0x2_0000_0000` (8GiB) [^2]  | User stacks                         |
//! | `0x7FFD_FFFF_F000`      | ? (dynamic)  | `0x2_0000_0000` (8GiB)       | User environment                    |
//! | `0x8000_0000_0000`      | -            | -                            | Non canonical addresses             |
//! | `0xFFFF_8000_0000_0000` | `0x0`        | `0x7FFD_8000_0000` (~128TiB) | Higher half direct mapping          |
//! | `0xFFFF_FFFD_8000_0000` | ? (dynamic)  | `0x2_0000_0000` (8GiB)       | Kernel stacks                       |
//! | `0xFFFF_FFFF_8000_0000` | ? (dynamic)  | `0x7FFF_F000` (~2GiB)        | Kernel executable                   |
//! | `0xFFFF_FFFF_FFFF_F000` | ? (dynamic)  | `0x1000` (1KiB)              | Current address space [^3]          |
//!
//! [^1]: [`USER_HEAP_TOP`]
//! [^2]: [`VIRT_STACK_SIZE_ALL`]
//! [^3]: the address space manager is of type [`Arc<AddressSpace>`]
//!
//! User and kernel stack spaces are split into stacks with the size of [`VIRT_STACK_SIZE`].

use alloc::collections::BTreeMap;
use core::{cmp::Ordering, ops::Range};

use hyperion_log::*;
use hyperion_mem::{
    from_higher_half, pmm, to_higher_half,
    vmm::{Handled, NotHandled, PageFaultResult, PageMapImpl, Privilege},
};
use spin::RwLock;
use x86_64::{
    registers::control::{Cr3, Cr3Flags},
    structures::paging::{
        mapper::{
            MapToError, MappedFrame, MapperFlush, MapperFlushAll, TranslateResult, UnmapError,
        },
        page_table::{FrameError, PageTableEntry},
        MappedPageTable, Mapper, OffsetPageTable, Page, PageSize, PageTable, PageTableFlags,
        PhysFrame, Size1GiB, Size2MiB, Size4KiB, Translate,
    },
    PhysAddr, VirtAddr,
};
#[allow(unused)] // for rustdoc
use {
    crate::stack::{AddressSpace, USER_HEAP_TOP, VIRT_STACK_SIZE, VIRT_STACK_SIZE_ALL},
    alloc::sync::Arc,
};

use super::pmm::Pfa;
use crate::paging::{Level4, WalkTableIterResult};

//

pub const HIGHER_HALF_DIRECT_MAPPING: VirtAddr = VirtAddr::new_truncate(0xFFFF_8000_0000_0000);
pub const KERNEL_STACKS: VirtAddr = VirtAddr::new_truncate(0xFFFF_FFFD_8000_0000);
pub const KERNEL_EXECUTABLE: VirtAddr = VirtAddr::new_truncate(0xFFFF_FFFF_8000_0000);
pub const CURRENT_ADDRESS_SPACE: VirtAddr = VirtAddr::new_truncate(0xFFFF_FFFF_FFFF_F000);

/// the page should not be freed
pub const NO_FREE: PageTableFlags = PageTableFlags::BIT_9;
/// the page is shared and was originally writeable
pub const COW: PageTableFlags = PageTableFlags::BIT_10;
/// the page is allocated on first use using a page fault
pub const LAZY_ALLOC: PageTableFlags = PageTableFlags::BIT_52;

//

fn v_addr_from_parts(
    offset: usize,
    p1_index: usize,
    p2_index: usize,
    p3_index: usize,
    p4_index: usize,
) -> VirtAddr {
    assert!(p4_index < (1 << 9));
    assert!(p3_index < (1 << 9));
    assert!(p2_index < (1 << 9));
    assert!(p1_index < (1 << 9));
    assert!(offset < (1 << 12));
    VirtAddr::new_truncate(
        (p4_index as u64) << 12 << 9 << 9 << 9
            | (p3_index as u64) << 12 << 9 << 9
            | (p2_index as u64) << 12 << 9
            | (p1_index as u64) << 12
            | (offset as u64),
    )
}

fn next_table(entry: &mut PageTableEntry) -> Option<&mut PageTable> {
    let frame = entry.frame().ok()?;
    Some(unsafe { &mut *to_higher_half(frame.start_address()).as_mut_ptr() })
}

fn page_fault_1gib(_entry: &mut PageTableEntry, _addr: VirtAddr) -> PageFaultResult {
    Ok(NotHandled)
}

fn page_fault_2mib(_entry: &mut PageTableEntry, _addr: VirtAddr) -> PageFaultResult {
    Ok(NotHandled)
}

fn page_fault_4kib(entry: &mut PageTableEntry, addr: VirtAddr) -> PageFaultResult {
    let mut flags = entry.flags();
    if flags.contains(COW) {
        flags.remove(COW);
        flags.insert(PageTableFlags::WRITABLE);
    } else {
        return Ok(NotHandled);
    }

    let page = Page::containing_address(addr);
    let frame = entry.frame().unwrap();
    let new_frame = unsafe { pmm::PFA.fork_page_fault(frame, page) };
    entry.set_frame(new_frame, flags);
    MapperFlush::new(Page::<Size4KiB>::containing_address(addr)).flush();

    Err(Handled)
}

//

pub struct PageMap {
    offs: RwLock<OffsetPageTable<'static>>,
    owned: bool,
}

//

impl PageMapImpl for PageMap {
    fn page_fault(&self, v_addr: VirtAddr, _privilege: Privilege) -> PageFaultResult {
        // TODO: lazy allocs

        let mut offs = self.offs.write();
        let l4 = offs.level_4_table();

        // giant pages
        let l4e = &mut l4[v_addr.p4_index()];
        let Some(l3) = next_table(l4e) else {
            return Ok(NotHandled);
        };

        // huge pages
        let l3e = &mut l3[v_addr.p3_index()];
        let Some(l2) = next_table(l3e) else {
            return page_fault_1gib(l3e, v_addr);
        };

        // normal pages
        let l2e = &mut l2[v_addr.p2_index()];
        let Some(l1) = next_table(l2e) else {
            return page_fault_2mib(l2e, v_addr);
        };

        let l1e = &mut l1[v_addr.p1_index()];
        page_fault_4kib(l1e, v_addr)
    }

    fn current() -> Self {
        // TODO: unsound, multiple mutable references to the same table could be made

        let (l4, _) = Cr3::read();
        let virt = to_higher_half(l4.start_address());
        let table: *mut PageTable = virt.as_mut_ptr();
        let table = unsafe { &mut *table };

        let offs =
            unsafe { OffsetPageTable::new(table, VirtAddr::new(hyperion_boot::hhdm_offset())) };
        let offs = RwLock::new(offs);

        Self { offs, owned: false }
    }

    fn new() -> Self {
        let frame = pmm::PFA.alloc(1);
        let new_table: &mut PageTable = unsafe { &mut *frame.virtual_addr().as_mut_ptr() };

        new_table.zero();

        let offs =
            unsafe { OffsetPageTable::new(new_table, VirtAddr::new(hyperion_boot::hhdm_offset())) };

        // TODO: Copy on write maps

        let offs = RwLock::new(offs);
        let page_map = Self { offs, owned: true };

        // hyperion_log::debug!("higher half direct map");
        // TODO: pmm::PFA.allocations();
        assert_eq!(
            HIGHER_HALF_DIRECT_MAPPING.as_u64(),
            hyperion_boot::hhdm_offset()
        );
        page_map.map(
            HIGHER_HALF_DIRECT_MAPPING..HIGHER_HALF_DIRECT_MAPPING + 0x100000000u64, // KERNEL_STACKS,
            PhysAddr::new(0x0),
            PageTableFlags::PRESENT | PageTableFlags::WRITABLE | PageTableFlags::NO_EXECUTE,
        );

        // FIXME: less dumb kernel mapping
        // deep copy the kernel mapping from the bootloader pagemap
        // and then use it as a global or CoW page map
        //
        // currently the whole kernel mapping is RW so a bug could write
        // code into the executable kernel region (which is obviously really fkng bad)
        let kernel = VirtAddr::new(hyperion_boot::virt_addr() as _);
        let top = kernel + hyperion_boot::size();
        hyperion_log::trace!("kernel map {kernel:#018x}");
        page_map.map(
            kernel..top,
            PhysAddr::new(hyperion_boot::phys_addr() as _),
            PageTableFlags::PRESENT | PageTableFlags::WRITABLE,
        );

        page_map
    }

    fn fork(&self) -> Self {
        let new = Self::new();

        assert!(self.is_active());

        let mut offs = self.offs.write();
        // TODO: CoW page tables also

        let hhdm_p4_index: usize = VirtAddr::new(hyperion_boot::hhdm_offset())
            .p4_index()
            .into();

        // TODO: iter maps instead of this mess
        let l4: &mut PageTable = offs.level_4_table();
        for (l4i, l4e) in l4.iter_mut().enumerate() {
            if l4i >= hhdm_p4_index {
                break;
            }

            let l3f = l4e.flags();
            let l3 = match l4e.frame() {
                Err(FrameError::FrameNotPresent) => continue,
                Err(FrameError::HugeFrame) => unreachable!(),
                Ok(l3) => l3,
            };
            let l3: &mut PageTable =
                unsafe { &mut *to_higher_half(l3.start_address()).as_mut_ptr() };
            for (l3i, l3e) in l3.iter_mut().enumerate() {
                let mut l2f = l3e.flags();
                let l2 = match l3e.frame() {
                    Err(FrameError::FrameNotPresent) => continue,
                    Err(FrameError::HugeFrame) => {
                        /* // 1 GiB page
                        // mark as read only
                        let w = l2f.contains(PageTableFlags::WRITABLE);
                        l2f.remove(PageTableFlags::WRITABLE);
                        l2f.insert(COW); // bit 10 == copy on write marker
                        l2f.set(COW_WRITEABLE, w); // bit 11 == copy on write writeable marker
                        l3e.set_flags(l2f);

                        let start = v_addr_from_parts(0, 0, 0, l3i, l4i);
                        new.map(start..start + Size1GiB::SIZE, l3e.addr(), l2f);
                        continue; */
                        todo!()
                    }
                    Ok(l2) => l2,
                };
                let l2: &mut PageTable =
                    unsafe { &mut *to_higher_half(l2.start_address()).as_mut_ptr() };
                for (l2i, l2e) in l2.iter_mut().enumerate() {
                    let mut l1f = l2e.flags();
                    let l1 = match l2e.frame() {
                        Err(FrameError::FrameNotPresent) => continue,
                        Err(FrameError::HugeFrame) => {
                            /* // 2 MiB page
                            // mark as read only
                            let w = l1f.contains(PageTableFlags::WRITABLE);
                            l1f.remove(PageTableFlags::WRITABLE);
                            l1f.insert(COW);
                            l1f.set(COW_WRITEABLE, w);
                            l2e.set_flags(l1f);

                            let start = v_addr_from_parts(0, 0, l2i, l3i, l4i);
                            new.map(start..start + Size2MiB::SIZE, l2e.addr(), l1f);
                            continue; */
                            todo!()
                        }
                        Ok(l1) => l1,
                    };
                    let l1: &mut PageTable =
                        unsafe { &mut *to_higher_half(l1.start_address()).as_mut_ptr() };
                    for (l1i, l1e) in l1.iter_mut().enumerate() {
                        let l0 = match l1e.frame() {
                            Err(FrameError::FrameNotPresent) => continue,
                            Err(FrameError::HugeFrame) => {
                                unreachable!()
                            }
                            Ok(l0) => l0,
                        };

                        // 4 KiB page
                        // mark as read only
                        let mut l0f = l1e.flags();
                        if l0f.contains(PageTableFlags::WRITABLE) {
                            l0f.remove(PageTableFlags::WRITABLE);
                            l0f.insert(COW);
                        }
                        l1e.set_flags(l0f);

                        let start = v_addr_from_parts(0, l1i, l2i, l3i, l4i);
                        let l1e_addr =
                            unsafe { pmm::PFA.fork(l0, Page::from_start_address(start).unwrap()) };
                        new.map(start..start + Size4KiB::SIZE, l1e_addr.start_address(), l0f);
                    }
                }
            }
        }

        MapperFlushAll::new().flush_all();

        new
    }

    fn activate(&self) {
        let mut offs = self.offs.write();

        let virt = offs.level_4_table() as *mut PageTable as *const () as u64;
        let phys = from_higher_half(VirtAddr::new(virt));
        let cr3 = PhysFrame::containing_address(phys);

        if Cr3::read().0 == cr3 {
            hyperion_log::trace!("page map switch avoided (same)");
            return;
        }

        hyperion_log::trace!("switching page maps");
        unsafe { Cr3::write(cr3, Cr3Flags::empty()) };
    }

    fn virt_to_phys(&self, addr: VirtAddr) -> Option<PhysAddr> {
        self.offs.read().translate_addr(addr)
    }

    fn phys_to_virt(&self, addr: PhysAddr) -> VirtAddr {
        to_higher_half(addr)
    }

    fn map(&self, v_addr: Range<VirtAddr>, mut p_addr: PhysAddr, flags: PageTableFlags) {
        if !v_addr.start.is_aligned(Size4KiB::SIZE) || !p_addr.is_aligned(Size4KiB::SIZE) {
            panic!("Not aligned");
        }

        let mut table = self.offs.write();
        let table = &mut table; // to make the formatting nicer

        let Range { mut start, end } = v_addr;
        let mut size;

        hyperion_log::trace!(
            "mapping [ 0x{start:016x}..0x{end:016x} ] to 0x{p_addr:016x} with {flags:?}"
        );

        loop {
            /* 'try_map: {
                size = Size1GiB::SIZE;
                match try_map_sized::<Size1GiB>(table, start, end, p_addr, flags, pmm) {
                    Ok(_) => break 'try_map,
                    Err(TryMapSizedError::MapToError(MapToError::PageAlreadyMapped(p))) => {
                        if p_addr == p.start_address() {
                            break 'try_map;
                        }

                        try_unmap_sized::<Size1GiB>(table, start, end)
                            .expect("to be able to unmap a page that had to be remapped");
                        try_map_sized::<Size1GiB>(table, start, end, p_addr, flags, pmm)
                            .expect("to be able to map a page after the error was resolved")
                    }
                    Err(_) => {}
                }
            } */

            if try_map_sized::<Size1GiB>(table, start, end, p_addr, flags)
                // .map_err(|err| hyperion_log::debug!("1GiB map err: {err:?}"))
                .is_ok()
            {
                size = Size1GiB::SIZE;
            } else if try_map_sized::<Size2MiB>(table, start, end, p_addr, flags)
                // .map_err(|err| hyperion_log::debug!("2MiB map err: {err:?}"))
                .is_ok()
            {
                size = Size2MiB::SIZE;
            } else if try_map_sized::<Size4KiB>(table, start, end, p_addr, flags)
                // .map_err(|err| hyperion_log::debug!("4KiB map err: {err:?}"))
                .is_ok()
            {
                size = Size4KiB::SIZE;
            } else {
                hyperion_log::error!("FIXME: failed to map [ 0x{start:016x} to 0x{p_addr:016x} ]");
                hyperion_log::error!(
                    "1GiB: {:?}",
                    try_map_sized::<Size1GiB>(table, start, end, p_addr, flags)
                );
                hyperion_log::error!(
                    "2MiB: {:?}",
                    try_map_sized::<Size2MiB>(table, start, end, p_addr, flags)
                );
                hyperion_log::error!(
                    "4KiB: {:?}",
                    try_map_sized::<Size4KiB>(table, start, end, p_addr, flags)
                );
                size = Size4KiB::SIZE;
            }

            // hyperion_log::trace!("mapped 0x{size:0x}");

            if let (Some(next_start), Some(next_p_addr)) = (
                v_addr_checked_add(start, size),
                p_addr_checked_add(p_addr, size),
            ) {
                start = next_start;
                p_addr = next_p_addr;
            } else {
                return;
            };

            match start.cmp(&end) {
                Ordering::Equal => break,
                Ordering::Greater => {
                    hyperion_log::error!("FIXME: over-mapped");
                    break;
                }

                _ => {}
            }
        }
    }

    fn unmap(&self, v_addr: Range<VirtAddr>) {
        if !v_addr.start.is_aligned(Size4KiB::SIZE) {
            panic!("Not aligned");
        }

        let mut table = self.offs.write();
        let table = &mut table; // to make the formatting nicer

        let Range { mut start, end } = v_addr;
        let mut size;

        hyperion_log::trace!("unmapping [ 0x{start:016x}..0x{end:016x} ]");

        loop {
            // hyperion_log::debug!("unmapping {start:?}..{end:?}");

            if try_unmap_sized::<Size1GiB>(table, start, end).is_ok() {
                size = Size1GiB::SIZE;
            } else if try_unmap_sized::<Size2MiB>(table, start, end).is_ok() {
                size = Size2MiB::SIZE;
            } else if try_unmap_sized::<Size4KiB>(table, start, end).is_ok() {
                size = Size4KiB::SIZE;
            } else {
                hyperion_log::error!("FIXME: failed to unmap [ 0x{start:016x} ]");
                size = Size4KiB::SIZE;
            }

            if let Some(next_start) = v_addr_checked_add(start, size) {
                start = next_start;
            } else {
                return;
            };

            match start.cmp(&end) {
                Ordering::Equal => break,
                Ordering::Greater => {
                    hyperion_log::error!("FIXME: over-unmapped");
                    break;
                }
                _ => {}
            }
        }
    }

    fn is_mapped(&self, v_addr: Range<VirtAddr>, contains: PageTableFlags) -> bool {
        let offs = self.offs.write();

        let Range { mut start, end } = v_addr;
        let mut size;

        loop {
            let (frame, flags) = match offs.translate(start) {
                TranslateResult::Mapped { frame, flags, .. } => (frame, flags),
                TranslateResult::NotMapped => return false,
                TranslateResult::InvalidFrameAddress(err) => {
                    hyperion_log::error!("Invalid page table frame address: 0x{err:016x}");
                    return false;
                }
            };

            if !flags.contains(contains) {
                return false;
            }

            size = match frame {
                MappedFrame::Size4KiB(_) => Size4KiB::SIZE,
                MappedFrame::Size2MiB(_) => Size2MiB::SIZE,
                MappedFrame::Size1GiB(_) => Size1GiB::SIZE,
            };

            if let Some(next_start) = v_addr_checked_add(start, size) {
                start = next_start;
            } else {
                return true;
            }

            if start >= end {
                return true;
            }
        }
    }
}

impl PageMap {
    /// # Safety
    /// Unsafe if the page map was obtained with `PageMap::current`,
    /// the page table should have been owned by the bootloader if so.
    pub unsafe fn mark_owned(&mut self) {
        self.owned = true;
    }

    pub fn is_active(&self) -> bool {
        Cr3::read().0 == self.cr3()
    }

    pub fn cr3(&self) -> PhysFrame {
        let mut offs = self.offs.write();

        let virt = offs.level_4_table() as *mut PageTable as *const () as u64;
        let phys = from_higher_half(VirtAddr::new(virt));

        PhysFrame::containing_address(phys)
    }

    pub fn debug(&self) {
        fn travel_level(
            flags: PageTableFlags,
            l: WalkTableIterResult,
            output: &mut BTreeMap<u64, (PageTableFlags, u64)>,
            v_addr: usize,
        ) {
            match l {
                WalkTableIterResult::Size1GiB(_p_addr) => {
                    // output.insert(p_addr.as_u64(), (flags, Size1GiB::SIZE));
                    output.insert(v_addr as u64, (flags, Size1GiB::SIZE));
                }
                WalkTableIterResult::Size2MiB(_p_addr) => {
                    // output.insert(p_addr.as_u64(), (flags, Size2MiB::SIZE));
                    output.insert(v_addr as u64, (flags, Size2MiB::SIZE));
                }
                WalkTableIterResult::Size4KiB(_p_addr) => {
                    // output.insert(p_addr.as_u64(), (flags, Size4KiB::SIZE));
                    output.insert(v_addr as u64, (flags, Size4KiB::SIZE));
                }
                WalkTableIterResult::Level3(l3) => {
                    for (i, flags, entry) in l3.iter() {
                        travel_level(flags, entry, output, v_addr + (i << 12 << 9 << 9));
                    }
                }
                WalkTableIterResult::Level2(l2) => {
                    for (i, flags, entry) in l2.iter() {
                        travel_level(flags, entry, output, v_addr + (i << 12 << 9));
                    }
                }
                WalkTableIterResult::Level1(l1) => {
                    for (i, flags, entry) in l1.iter() {
                        travel_level(flags, entry, output, v_addr + (i << 12));
                    }
                }
            }
        }

        let hhdm_p4_index: usize = VirtAddr::new(hyperion_boot::hhdm_offset())
            .p4_index()
            .into();

        let mut offs = self.offs.write();

        println!("BEGIN PAGE TABLE ITER");
        let mut output = BTreeMap::new();
        let mut output_hh = BTreeMap::new();
        let l4 = Level4::from_pml4(offs.level_4_table());
        for (i, flags, entry) in l4.iter() {
            let v_addr = i << 12 << 9 << 9 << 9;
            if i < hhdm_p4_index {
                travel_level(flags, entry, &mut output, v_addr);
            } else {
                travel_level(flags, entry, &mut output_hh, v_addr);
            }
        }
        println!("END PAGE TABLE ITER");

        let print_output = |output: BTreeMap<u64, (PageTableFlags, u64)>| {
            let mut last = None;
            for (segment_start, (mut flags, segment_size)) in output {
                let segment_end = segment_start + segment_size;
                flags.remove(PageTableFlags::ACCESSED);
                flags.remove(PageTableFlags::DIRTY);
                flags.remove(PageTableFlags::HUGE_PAGE);

                if let Some((last_flags, last_start, last_end)) = last.take() {
                    if last_flags != flags || last_end < segment_start {
                        println!(
                            "PAGING SEGMENT [ 0x{last_start:016x}..0x{last_end:016x} ] {:?}",
                            last_flags
                        );
                        last = Some((flags, segment_start, segment_end));
                    } else {
                        last = Some((last_flags, last_start, segment_end));
                    }
                } else {
                    last = Some((flags, segment_start, segment_end));
                }
            }
            if let Some((last_flags, last_start, last_end)) = last {
                println!(
                    "PAGING SEGMENT [ 0x{last_start:016x}..0x{last_end:016x} ] {:?}",
                    last_flags
                );
            }
        };
        println!("BEGIN PAGE TABLE SEGMENTS");
        print_output(output);
        println!("BEGIN HIGHER HALF PAGE TABLE SEGMENTS");
        print_output(output_hh);
        println!("END PAGE TABLE SEGMENTS");
    }
}

impl Drop for PageMap {
    fn drop(&mut self) {
        fn travel_level(l: WalkTableIterResult) {
            match l {
                WalkTableIterResult::Size1GiB(_p_addr) => {}
                WalkTableIterResult::Size2MiB(_p_addr) => {}
                WalkTableIterResult::Size4KiB(_p_addr) => {}
                WalkTableIterResult::Level3(l3) => {
                    for (_, flags, entry) in l3.iter() {
                        if !flags.contains(NO_FREE) {
                            travel_level(entry);
                        }
                    }

                    let table = from_higher_half(VirtAddr::new(l3.0 as *const _ as u64));
                    Pfa.deallocate_frame(PhysFrame::containing_address(table));
                }
                WalkTableIterResult::Level2(l2) => {
                    for (_, flags, entry) in l2.iter() {
                        if !flags.contains(NO_FREE) {
                            travel_level(entry);
                        }
                    }

                    let table = from_higher_half(VirtAddr::new(l2.0 as *const _ as u64));
                    Pfa.deallocate_frame(PhysFrame::containing_address(table));
                }
                WalkTableIterResult::Level1(l1) => {
                    for (_, flags, entry) in l1.iter() {
                        if !flags.contains(NO_FREE) {
                            travel_level(entry);
                        }
                    }

                    let table = from_higher_half(VirtAddr::new(l1.0 as *const _ as u64));
                    Pfa.deallocate_frame(PhysFrame::containing_address(table));
                }
            }
        }

        if !self.owned {
            return;
        }

        assert!(!self.is_active());

        let offs = self.offs.get_mut();

        let l4 = Level4::from_pml4(offs.level_4_table());
        for (_, flags, entry) in l4.iter() {
            if !flags.contains(NO_FREE) {
                travel_level(entry);
            } else {
                hyperion_log::debug!("skip bit 9");
            }
        }

        let table = from_higher_half(VirtAddr::new(offs.level_4_table() as *const _ as u64));
        Pfa.deallocate_frame(PhysFrame::containing_address(table));
    }
}

/* fn lazy_map(
    table: &mut OffsetPageTable,
    v_addr: VirtAddr,
    region: Range<VirtAddr>,
    p_addr: PhysAddr,
    flags: PageTableFlags,
) -> PageFaultResult {
    if !region.contains(&v_addr) {
        return PageFaultResult::NotHandled;
    }

    if lazy_map_sized::<Size1GiB>(table, v_addr, region.clone(), p_addr, flags)
        == PageFaultResult::Handled
    {
        return PageFaultResult::Handled;
    }
    if lazy_map_sized::<Size2MiB>(table, v_addr, region.clone(), p_addr, flags)
        == PageFaultResult::Handled
    {
        return PageFaultResult::Handled;
    }
    if lazy_map_sized::<Size4KiB>(table, v_addr, region.clone(), p_addr, flags)
        == PageFaultResult::Handled
    {
        return PageFaultResult::Handled;
    }

    let _ = region;

    PageFaultResult::NotHandled
}

fn lazy_map_sized<T>(
    table: &mut OffsetPageTable,
    v_addr: VirtAddr,
    region: Range<VirtAddr>,
    p_addr: PhysAddr,
    flags: PageTableFlags,
) -> PageFaultResult
where
    T: PageSize + core::fmt::Debug,
    for<'a> OffsetPageTable<'a>: Mapper<T>,
{
    let map = v_addr.align_down(T::SIZE)..v_addr.align_up(T::SIZE);
    if !region.contains(&map.start) && region.contains(&map.end) {
        return PageFaultResult::NotHandled;
    }

    let p_addr = p_addr - map.start.as_u64();

    if let Err(err) = try_map_sized::<T>(table, map.start, map.end, p_addr, flags) {
        hyperion_log::error!("map err: {err:?}");
        return PageFaultResult::NotHandled;
    }

    PageFaultResult::Handled
} */

#[derive(Debug)]
pub enum TryMapSizedError<T: PageSize> {
    Overflow,
    NotAligned,
    MapToError(MapToError<T>),
    WrongSize,
}

fn try_map_sized<T>(
    table: &mut OffsetPageTable,
    start: VirtAddr,
    end: VirtAddr,
    p_addr: PhysAddr,
    flags: PageTableFlags,
) -> Result<(), TryMapSizedError<T>>
where
    T: PageSize,
    for<'a> OffsetPageTable<'a>: Mapper<T>,
{
    let Some(mapping_end) = start.as_u64().checked_add(T::SIZE - 1) else {
        return Err(TryMapSizedError::Overflow);
    };

    if mapping_end > end.as_u64() {
        return Err(TryMapSizedError::Overflow);
    }

    if !start.is_aligned(T::SIZE) || !p_addr.is_aligned(T::SIZE) {
        return Err(TryMapSizedError::NotAligned);
    }

    let page = Page::<T>::containing_address(start);
    let frame = PhysFrame::<T>::containing_address(p_addr);

    let result = unsafe {
        table.map_to_with_table_flags(
            page,
            frame,
            flags,
            (flags & (PageTableFlags::PRESENT | PageTableFlags::USER_ACCESSIBLE))
                | PageTableFlags::WRITABLE,
            &mut Pfa,
        )
    };

    if let Err(MapToError::PageAlreadyMapped(old_frame)) = result {
        if old_frame == frame {
            return Ok(());
        }
    }

    result
        .map_err(|err| TryMapSizedError::MapToError(err))?
        .flush();

    /* hyperion_log::debug!("mapped 1GiB at 0x{:016x}", start);
    crash_after_nth(10); */

    Ok(())
}

fn try_unmap_sized<T>(
    table: &mut OffsetPageTable,
    start: VirtAddr,
    _end: VirtAddr,
) -> Result<(), TryMapSizedError<T>>
where
    T: PageSize,
    for<'a> OffsetPageTable<'a>: Mapper<T>,
{
    let Some(_mapping_end) = start.as_u64().checked_add(T::SIZE - 1) else {
        return Err(TryMapSizedError::Overflow);
    };

    /* if mapping_end > end.as_u64() {
        return Err(TryMapSizedError::Overflow);
    } */

    if !start.is_aligned(T::SIZE) {
        return Err(TryMapSizedError::NotAligned);
    }

    let page = Page::<T>::containing_address(start);

    match table.unmap(page) {
        Ok((_, ok)) => {
            ok.flush();
            Ok(())
        }
        Err(UnmapError::PageNotMapped) => {
            // hyperion_log::debug!("already not mapped");
            Ok(())
        }
        Err(UnmapError::ParentEntryHugePage) => Err(TryMapSizedError::WrongSize),
        Err(_err) => panic!("{_err:?}"),
    }
}

fn v_addr_checked_add(addr: VirtAddr, rhs: u64) -> Option<VirtAddr> {
    VirtAddr::try_new(addr.as_u64().checked_add(rhs)?).ok()
}

fn p_addr_checked_add(addr: PhysAddr, rhs: u64) -> Option<PhysAddr> {
    PhysAddr::try_new(addr.as_u64().checked_add(rhs)?).ok()
}
