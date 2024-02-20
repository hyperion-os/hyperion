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
use core::ops::Range;

use hyperion_log::*;
use hyperion_mem::{
    from_higher_half,
    pmm::{self},
    to_higher_half,
    vmm::{Handled, NotHandled, PageFaultResult, PageMapImpl, Privilege},
};
use spin::RwLock;
use x86_64::{
    instructions::tlb,
    registers::control::{Cr3, Cr3Flags},
    structures::paging::{
        mapper::{MapToError, MappedFrame, MapperFlush, MapperFlushAll},
        page_table::{FrameError, PageTableEntry},
        Page, PageSize, PageTable, PageTableFlags, PhysFrame, Size1GiB, Size2MiB, Size4KiB,
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

fn page_fault_1gib(entry: &mut PageTableEntry, addr: VirtAddr) -> PageFaultResult {
    let mut flags = entry.flags();

    if flags.contains(COW) {
        todo!()
    } else if flags.contains(LAZY_ALLOC) {
        flags.remove(PageTableFlags::HUGE_PAGE);

        // convert the 1gib page into 2mib pages and allocate only one of them now

        let l3 = LockedPageMap::create_table(entry).unwrap();
        for l3e in l3.iter_mut() {
            l3e.set_flags(flags); // mark all with the original flags
        }
        let l3e = &mut l3[addr.p3_index()];

        return page_fault_4kib(l3e, addr);
    }

    Ok(NotHandled)
}

fn page_fault_2mib(entry: &mut PageTableEntry, addr: VirtAddr) -> PageFaultResult {
    let mut flags = entry.flags();

    if flags.contains(COW) {
        todo!()
    } else if flags.contains(LAZY_ALLOC) {
        flags.remove(PageTableFlags::HUGE_PAGE);

        // convert the 2mib page into 4kib pages and allocate only one of them now

        let l2 = LockedPageMap::create_table(entry).unwrap();
        for l2e in l2.iter_mut() {
            l2e.set_flags(flags); // mark all with the original flags
        }
        let l2e = &mut l2[addr.p2_index()];

        return page_fault_4kib(l2e, addr);
    }

    Ok(NotHandled)
}

fn page_fault_4kib(entry: &mut PageTableEntry, addr: VirtAddr) -> PageFaultResult {
    let frame;
    let mut flags = entry.flags();

    if flags.contains(COW) {
        // handle a fork CoW
        flags.remove(COW);
        flags.insert(PageTableFlags::WRITABLE);

        let page = Page::containing_address(addr);
        frame = unsafe { pmm::PFA.fork_page_fault(entry.frame().unwrap(), page) };
    } else if flags.contains(LAZY_ALLOC) {
        // handle a lazy alloc
        flags.remove(LAZY_ALLOC);
        flags.insert(PageTableFlags::PRESENT);

        frame = alloc_table();
    } else {
        return Ok(NotHandled);
    }

    entry.set_frame(frame, flags);
    MapperFlush::new(Page::<Size4KiB>::containing_address(addr)).flush();

    Err(Handled)
}

fn alloc_table() -> PhysFrame {
    let frame = pmm::PFA.alloc(1);
    PhysFrame::<Size4KiB>::from_start_address(frame.physical_addr()).unwrap()
}

// FIXME: vmm dealloc
/* fn free_table(f: PhysFrame) {
    let frame = unsafe { PageFrame::new(f.start_address(), 1) };
    pmm::PFA.free(frame)
} */

//

pub struct PageMap {
    inner: RwLock<LockedPageMap>,
    owned: bool,
}

impl PageMapImpl for PageMap {
    fn page_fault(&self, v_addr: VirtAddr, _privilege: Privilege) -> PageFaultResult {
        let mut inner = self.inner.write();
        let l4 = inner.l4();

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
        let l4: &mut PageTable = unsafe { &mut *to_higher_half(l4.start_address()).as_mut_ptr() };

        Self {
            inner: RwLock::new(LockedPageMap { l4 }),
            owned: false,
        }
    }

    fn new() -> Self {
        let cr3 = alloc_table();
        let l4: &mut PageTable = unsafe { &mut *to_higher_half(cr3.start_address()).as_mut_ptr() };

        l4.zero();

        // TODO: Copy on write maps

        let page_map = Self {
            inner: RwLock::new(LockedPageMap { l4 }),
            owned: true,
        };

        // hyperion_log::debug!("higher half direct map");
        // TODO: pmm::PFA.allocations();
        assert_eq!(
            HIGHER_HALF_DIRECT_MAPPING.as_u64(),
            hyperion_boot::hhdm_offset()
        );
        page_map.map(
            HIGHER_HALF_DIRECT_MAPPING..HIGHER_HALF_DIRECT_MAPPING + 0x100000000u64, // KERNEL_STACKS,
            Some(PhysAddr::new(0x0)),
            PageTableFlags::WRITABLE | PageTableFlags::NO_EXECUTE,
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
            Some(PhysAddr::new(hyperion_boot::phys_addr() as _)),
            PageTableFlags::WRITABLE,
        );

        // page_map.debug();

        page_map
    }

    fn fork(&self) -> Self {
        let new = Self::new();

        assert!(self.is_active());

        let mut inner = self.inner.write();
        // TODO: CoW page tables also

        let hhdm_p4_index: usize = VirtAddr::new(hyperion_boot::hhdm_offset())
            .p4_index()
            .into();

        // TODO: iter maps instead of this mess
        let l4: &mut PageTable = inner.l4();
        for (l4i, l4e) in l4.iter_mut().enumerate() {
            if l4i >= hhdm_p4_index {
                break;
            }

            let l3 = match l4e.frame() {
                Err(FrameError::FrameNotPresent) => continue,
                Err(FrameError::HugeFrame) => unreachable!(),
                Ok(l3) => l3,
            };
            let l3: &mut PageTable =
                unsafe { &mut *to_higher_half(l3.start_address()).as_mut_ptr() };
            for (l3i, l3e) in l3.iter_mut().enumerate() {
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

                        let start = v_addr_from_parts(0, l1i, l2i, l3i, l4i);

                        // 4 KiB page
                        // mark as read only
                        let mut l0f = l1e.flags();
                        if l0f.contains(LAZY_ALLOC) {
                            new.map(start..start + Size4KiB::SIZE, None, l0f);
                            continue;
                        }

                        if l0f.contains(PageTableFlags::WRITABLE) {
                            l0f.remove(PageTableFlags::WRITABLE);
                            l0f.insert(COW);
                        }
                        l1e.set_flags(l0f);

                        let new_frame =
                            unsafe { pmm::PFA.fork(l0, Page::from_start_address(start).unwrap()) }
                                .start_address();
                        new.map(start..start + Size4KiB::SIZE, Some(new_frame), l0f);
                    }
                }
            }
        }

        MapperFlushAll::new().flush_all();

        new
    }

    fn activate(&self) {
        self.inner.read().activate()
    }

    fn virt_to_phys(&self, addr: VirtAddr) -> Option<PhysAddr> {
        self.inner
            .read()
            .translate_addr(addr)
            .map(|(addr, _, _)| addr)
    }

    fn phys_to_virt(&self, addr: PhysAddr) -> VirtAddr {
        to_higher_half(addr)
    }

    fn map(&self, v_addr: Range<VirtAddr>, p_addr: Option<PhysAddr>, flags: PageTableFlags) {
        self.inner.write().map(v_addr, p_addr, flags)
    }

    fn unmap(&self, v_addr: Range<VirtAddr>) {
        self.inner.write().unmap(v_addr)
    }

    fn remap(&self, v_addr: Range<VirtAddr>, new_flags: PageTableFlags) {
        self.inner.write().remap(v_addr, new_flags)
    }

    fn is_mapped(&self, v_addr: Range<VirtAddr>, has_at_least: PageTableFlags) -> bool {
        self.inner.read().is_mapped(v_addr, has_at_least)
    }
}

//

struct LockedPageMap {
    l4: &'static mut PageTable,
}

impl LockedPageMap {
    fn l4(&mut self) -> &mut PageTable {
        self.l4
    }

    fn activate(&self) {
        let virt = self.l4 as *const PageTable as u64;
        let phys = from_higher_half(VirtAddr::new(virt));
        let cr3 = PhysFrame::containing_address(phys);

        if Cr3::read().0 == cr3 {
            hyperion_log::trace!("page map switch avoided (same)");
            return;
        }

        hyperion_log::trace!("switching page maps");
        unsafe { Cr3::write(cr3, Cr3Flags::empty()) };
    }

    fn map(
        &mut self,
        Range { mut start, end }: Range<VirtAddr>,
        mut to: Option<PhysAddr>,
        flags: PageTableFlags,
    ) {
        if !start.is_aligned(Size4KiB::SIZE)
            || !end.is_aligned(Size4KiB::SIZE)
            || !to.map_or(true, |to| to.is_aligned(Size4KiB::SIZE))
        {
            panic!("Not aligned");
        }

        if let Some(to) = to {
            hyperion_log::trace!(
                "mapping [ 0x{start:016x}..0x{end:016x} ] to 0x{to:016x} with {flags:?}"
            );
        } else {
            hyperion_log::trace!(
                "mapping [ 0x{start:016x}..0x{end:016x} ] to <lazy> with {flags:?}"
            );
        }

        loop {
            if start == end {
                break;
            } else if start > end {
                panic!("over-mapped");
            }

            let Err(err_1gib) = self.try_map_1gib(start..end, to, flags) else {
                // could crash if the last possible phys/virt page was mapped
                start += Size1GiB::SIZE;
                to = to.map(|to| to + Size1GiB::SIZE);
                continue;
            };

            let Err(err_2mib) = self.try_map_2mib(start..end, to, flags) else {
                start += Size2MiB::SIZE;
                to = to.map(|to| to + Size2MiB::SIZE);
                continue;
            };

            let Err(err_4kib) = self.try_map_4kib(start..end, to, flags) else {
                start += Size4KiB::SIZE;
                to = to.map(|to| to + Size4KiB::SIZE);
                continue;
            };

            if let Some(to) = to {
                hyperion_log::error!("FIXME: failed to map [ 0x{start:016x} to 0x{to:016x} ]");
            } else {
                hyperion_log::error!("FIXME: failed to map [ 0x{start:016x} to <lazy> ]");
            }
            hyperion_log::error!(" .. 1GiB: {err_1gib:?}");
            hyperion_log::error!(" .. 2MiB: {err_2mib:?}");
            hyperion_log::error!(" .. 4KiB: {err_4kib:?}");
            panic!();
        }
    }

    fn is_map_valid<S: PageSize>(
        Range { start, end }: Range<VirtAddr>,
    ) -> Result<Page<S>, TryMapError<S>> {
        let Ok(page) = Page::<S>::from_start_address(start) else {
            return Err(TryMapError::NotAligned);
        };

        let Some(limit) = end.as_u64().checked_sub(S::SIZE) else {
            return Err(TryMapError::Overflow);
        };

        if start.as_u64() > limit {
            return Err(TryMapError::Overflow);
        }

        Ok(page)
    }

    fn is_phys_map_valid<S: PageSize>(
        to: Option<PhysAddr>,
    ) -> Result<Option<PhysFrame<S>>, TryMapError<S>> {
        to.map(|to| PhysFrame::from_start_address(to).map_err(|_| TryMapError::NotAligned))
            .transpose()
    }

    // None = HugeFrame
    fn create_table(entry: &mut PageTableEntry) -> Option<&mut PageTable> {
        let flags =
            PageTableFlags::PRESENT | PageTableFlags::WRITABLE | PageTableFlags::USER_ACCESSIBLE;

        let table = match entry.frame() {
            Ok(table) => table,
            Err(FrameError::FrameNotPresent) => {
                let table = alloc_table();
                entry.set_frame(table, flags);
                table
            }
            Err(FrameError::HugeFrame) => return None,
        };

        let addr = to_higher_half(table.start_address()).as_mut_ptr();

        Some(unsafe { &mut *addr })
    }

    fn try_map_if_diff<S: PageSize>(
        entry: &mut PageTableEntry,
        to: Option<PhysFrame<S>>,
        flags: PageTableFlags,
    ) -> Result<(), TryMapError<S>> {
        let old: (PageTableFlags, PhysAddr) = (entry.flags(), entry.addr());
        let new: (PageTableFlags, PhysAddr);

        if let Some(to) = to {
            // map immediately
            new = (
                // prevent present maps to be marked as LAZY_ALLOC
                flags.difference(LAZY_ALLOC) | PageTableFlags::PRESENT,
                to.start_address(),
            );
        } else {
            // alloc lazily
            new = (
                // prevent lazy alloc maps to be marked as PRESENT
                flags.difference(PageTableFlags::PRESENT) | LAZY_ALLOC,
                PhysAddr::new_truncate(0),
            );
        }

        if old == new {
            // already mapped but it is already correct
            return Ok(());
        }

        if !entry.is_unused() {
            return Err(TryMapError::AlreadyMapped);
        }

        entry.set_addr(new.1, new.0);
        Ok(())
    }

    fn try_map_1gib(
        &mut self,
        Range { start, end }: Range<VirtAddr>,
        to: Option<PhysAddr>,
        flags: PageTableFlags,
    ) -> Result<(), TryMapError<Size1GiB>> {
        let from = Self::is_map_valid(start..end)?;
        let to = Self::is_phys_map_valid(to)?;

        let Some(p3) = Self::create_table(&mut self.l4[from.p4_index()]) else {
            unreachable!("512GiB maps are not supported");
        };
        let p3e = &mut p3[from.p3_index()];

        Self::try_map_if_diff(p3e, to, flags | PageTableFlags::HUGE_PAGE)?;
        tlb::flush(from.start_address());

        Ok(())
    }

    fn try_map_2mib(
        &mut self,
        Range { start, end }: Range<VirtAddr>,
        to: Option<PhysAddr>,
        flags: PageTableFlags,
    ) -> Result<(), TryMapError<Size2MiB>> {
        let from = Self::is_map_valid(start..end)?;
        let to = Self::is_phys_map_valid(to)?;

        let Some(p3) = Self::create_table(&mut self.l4[from.p4_index()]) else {
            unreachable!("512GiB maps are not supported");
        };
        let Some(p2) = Self::create_table(&mut p3[from.p3_index()]) else {
            return Err(TryMapError::WrongSize);
        };
        let p2e = &mut p2[from.p2_index()];

        Self::try_map_if_diff(p2e, to, flags | PageTableFlags::HUGE_PAGE)?;
        tlb::flush(from.start_address());

        Ok(())
    }

    fn try_map_4kib(
        &mut self,
        Range { start, end }: Range<VirtAddr>,
        to: Option<PhysAddr>,
        flags: PageTableFlags,
    ) -> Result<(), TryMapError<Size4KiB>> {
        let from = Self::is_map_valid(start..end)?;
        let to = Self::is_phys_map_valid(to)?;

        let Some(p3) = Self::create_table(&mut self.l4[from.p4_index()]) else {
            unreachable!("512GiB maps are not supported");
        };
        let Some(p2) = Self::create_table(&mut p3[from.p3_index()]) else {
            return Err(TryMapError::WrongSize);
        };
        let Some(p1) = Self::create_table(&mut p2[from.p2_index()]) else {
            return Err(TryMapError::WrongSize);
        };
        let p1e = &mut p1[from.p1_index()];

        Self::try_map_if_diff(p1e, to, flags)?;
        tlb::flush(from.start_address());

        Ok(())
    }

    fn unmap(&mut self, Range { mut start, end }: Range<VirtAddr>) {
        if !start.is_aligned(Size4KiB::SIZE) || !end.is_aligned(Size4KiB::SIZE) {
            panic!("Not aligned");
        }

        hyperion_log::trace!("unmapping [ 0x{start:016x}..0x{end:016x} ]");

        loop {
            if start == end {
                break;
            } else if start > end {
                panic!("over-unmapped");
            }

            hyperion_log::trace!("unmapping {start:#018x}");

            let Err(err_1gib) = self.try_unmap_1gib(start..end) else {
                // could crash if the last possible phys/virt page was mapped
                start += Size1GiB::SIZE;
                continue;
            };

            let Err(err_2mib) = self.try_unmap_2mib(start..end) else {
                start += Size2MiB::SIZE;
                continue;
            };

            let Err(err_4kib) = self.try_unmap_4kib(start..end) else {
                start += Size4KiB::SIZE;
                continue;
            };

            hyperion_log::error!("FIXME: failed to unmap [ 0x{start:016x} ]");
            hyperion_log::error!(" .. 1GiB: {err_1gib:?}");
            hyperion_log::error!(" .. 2MiB: {err_2mib:?}");
            hyperion_log::error!(" .. 4KiB: {err_4kib:?}");
            panic!();
        }
    }

    // None = HugeFrame
    fn read_table<S: PageSize>(
        entry: &mut PageTableEntry,
    ) -> Result<Option<&mut PageTable>, TryMapError<S>> {
        match entry.frame() {
            Ok(table) => {
                let addr = to_higher_half(table.start_address()).as_mut_ptr();
                Ok(Some(unsafe { &mut *addr }))
            }
            Err(FrameError::FrameNotPresent) => Ok(None),
            Err(FrameError::HugeFrame) => Err(TryMapError::WrongSize),
        }
    }

    fn try_unmap_if_correct_size<S: PageSize>(
        entry: &mut PageTableEntry,
        should_be_huge_page: bool,
    ) -> Result<(), TryMapError<S>> {
        if entry.is_unused() {
            return Ok(());
        }

        if entry.flags().contains(PageTableFlags::HUGE_PAGE) != should_be_huge_page {
            return Err(TryMapError::WrongSize);
        }

        // free_table(PhysFrame::from_start_address(p3e.addr()).unwrap());
        entry.set_unused();
        Ok(())
    }

    fn try_unmap_1gib(
        &mut self,
        Range { start, end }: Range<VirtAddr>,
    ) -> Result<(), TryMapError<Size1GiB>> {
        let from = Self::is_map_valid(start..end)?;

        let Some(p3) = Self::read_table(&mut self.l4[from.p4_index()])? else {
            unreachable!("512GiB maps are not supported");
        };
        let p3e = &mut p3[from.p3_index()];

        Self::try_unmap_if_correct_size(p3e, true)?;
        tlb::flush(from.start_address());

        Ok(())
    }

    fn try_unmap_2mib(
        &mut self,
        Range { start, end }: Range<VirtAddr>,
    ) -> Result<(), TryMapError<Size2MiB>> {
        let from = Self::is_map_valid(start..end)?;

        let Some(p3) = Self::read_table(&mut self.l4[from.p4_index()])? else {
            unreachable!("512GiB maps are not supported");
        };
        let Some(p2) = Self::read_table(&mut p3[from.p3_index()])? else {
            return Ok(());
        };
        let p2e = &mut p2[from.p2_index()];

        Self::try_unmap_if_correct_size(p2e, true)?;
        tlb::flush(from.start_address());

        Ok(())
    }

    fn try_unmap_4kib(
        &mut self,
        Range { start, end }: Range<VirtAddr>,
    ) -> Result<(), TryMapError<Size4KiB>> {
        let from = Self::is_map_valid(start..end)?;

        let Some(p3) = Self::read_table(&mut self.l4[from.p4_index()])? else {
            return Ok(());
        };
        let Some(p2) = Self::read_table(&mut p3[from.p3_index()])? else {
            return Ok(());
        };
        let Some(p1) = Self::read_table(&mut p2[from.p2_index()])? else {
            return Ok(());
        };
        let p1e = &mut p1[from.p1_index()];

        Self::try_unmap_if_correct_size(p1e, false)?;
        tlb::flush(from.start_address());

        Ok(())
    }

    fn remap(&mut self, Range { mut start, end }: Range<VirtAddr>, new_flags: PageTableFlags) {
        if !start.is_aligned(Size4KiB::SIZE) || !end.is_aligned(Size4KiB::SIZE) {
            panic!("Not aligned");
        }

        hyperion_log::trace!("remapping [ 0x{start:016x}..0x{end:016x} ] with {new_flags:?}");

        loop {
            if start == end {
                break;
            } else if start > end {
                panic!("over-unmapped");
            }

            hyperion_log::trace!("remapping {start:#018x}");

            let Err(err_1gib) = self.try_remap_1gib(start..end, new_flags) else {
                // could crash if the last possible phys/virt page was mapped
                start += Size1GiB::SIZE;
                continue;
            };

            let Err(err_2mib) = self.try_remap_2mib(start..end, new_flags) else {
                start += Size2MiB::SIZE;
                continue;
            };

            let Err(err_4kib) = self.try_remap_4kib(start..end, new_flags) else {
                start += Size4KiB::SIZE;
                continue;
            };

            hyperion_log::error!("FIXME: failed to remap [ 0x{start:016x} ]");
            hyperion_log::error!(" .. 1GiB: {err_1gib:?}");
            hyperion_log::error!(" .. 2MiB: {err_2mib:?}");
            hyperion_log::error!(" .. 4KiB: {err_4kib:?}");
            panic!();
        }
    }

    fn try_remap<S: PageSize>(
        entry: &mut PageTableEntry,
        mut flags: PageTableFlags,
        addr: VirtAddr,
    ) -> Result<(), TryMapError<S>> {
        if entry.is_unused() {
            return Err(TryMapError::NotMapped);
        }

        flags.insert(entry.flags().intersection(
            PageTableFlags::PRESENT
                | PageTableFlags::ACCESSED
                | PageTableFlags::DIRTY
                | NO_FREE
                | COW
                | LAZY_ALLOC,
        ));

        if entry.flags() == flags {
            return Ok(());
        }

        // hyperion_log::debug!(
        //     "remapped {addr:#018x} as {flags:?} (old:{:?})",
        //     entry.flags()
        // );
        entry.set_flags(flags);
        tlb::flush(addr);

        Ok(())
    }

    fn try_remap_1gib(
        &mut self,
        Range { start, end }: Range<VirtAddr>,
        flags: PageTableFlags,
    ) -> Result<(), TryMapError<Size1GiB>> {
        let from = Self::is_map_valid(start..end)?;

        let Some(p3) = Self::read_table(&mut self.l4[from.p4_index()])? else {
            return Ok(());
        };
        let p3e = &mut p3[from.p3_index()];

        if !flags.contains(PageTableFlags::HUGE_PAGE) {
            return Err(TryMapError::WrongSize);
        }

        Self::try_remap(p3e, flags | PageTableFlags::HUGE_PAGE, from.start_address())?;

        Ok(())
    }

    fn try_remap_2mib(
        &mut self,
        Range { start, end }: Range<VirtAddr>,
        flags: PageTableFlags,
    ) -> Result<(), TryMapError<Size2MiB>> {
        let from = Self::is_map_valid(start..end)?;

        let Some(p3) = Self::read_table(&mut self.l4[from.p4_index()])? else {
            return Ok(());
        };
        let Some(p2) = Self::read_table(&mut p3[from.p3_index()])? else {
            return Ok(());
        };
        let p2e = &mut p2[from.p2_index()];

        if !flags.contains(PageTableFlags::HUGE_PAGE) {
            return Err(TryMapError::WrongSize);
        }

        Self::try_remap(p2e, flags | PageTableFlags::HUGE_PAGE, from.start_address())?;

        Ok(())
    }

    fn try_remap_4kib(
        &mut self,
        Range { start, end }: Range<VirtAddr>,
        flags: PageTableFlags,
    ) -> Result<(), TryMapError<Size4KiB>> {
        let from = Self::is_map_valid(start..end)?;

        let Some(p3) = Self::read_table(&mut self.l4[from.p4_index()])? else {
            return Ok(());
        };
        let Some(p2) = Self::read_table(&mut p3[from.p3_index()])? else {
            return Ok(());
        };
        let Some(p1) = Self::read_table(&mut p2[from.p2_index()])? else {
            return Ok(());
        };
        let p1e = &mut p1[from.p1_index()];

        if flags.contains(PageTableFlags::HUGE_PAGE) {
            return Err(TryMapError::WrongSize);
        }

        Self::try_remap(p1e, flags, from.start_address())?;

        Ok(())
    }

    fn is_mapped(
        &self,
        Range { mut start, mut end }: Range<VirtAddr>,
        contains: PageTableFlags,
    ) -> bool {
        start = start.align_down(Size4KiB::SIZE);
        end = end.align_up(Size4KiB::SIZE);

        loop {
            if start >= end {
                return true;
            }

            let l4 = &self.l4[start.p4_index()];
            if !self.is_mapped_layer(l4, contains) {
                return false;
            }

            let l3 = match self.translate_layer(l4) {
                Some(Ok(next)) => &next[start.p3_index()],
                Some(Err(())) => unreachable!(),
                None => return false,
            };
            if !self.is_mapped_layer(l3, contains) {
                return false;
            }

            let l2 = match self.translate_layer(l3) {
                Some(Ok(next)) => &next[start.p2_index()],
                Some(Err(())) => {
                    // giant page
                    if !l3.flags().contains(contains) {
                        return false;
                    }

                    start += Size1GiB::SIZE;
                    continue;
                }
                None => return false,
            };
            if !self.is_mapped_layer(l2, contains) {
                return false;
            }

            let l1 = match self.translate_layer(l2) {
                Some(Ok(next)) => &next[start.p1_index()],
                Some(Err(())) => {
                    // huge page
                    if !l2.flags().contains(contains) {
                        return false;
                    }

                    start += Size2MiB::SIZE;
                    continue;
                }
                None => return false,
            };
            if !self.is_mapped_layer(l1, contains) {
                return false;
            }

            if !l2.flags().contains(contains) {
                return false;
            }

            start += Size4KiB::SIZE;
        }
    }

    fn is_mapped_layer(&self, entry: &PageTableEntry, flags: PageTableFlags) -> bool {
        let lf = entry.flags();
        if lf.contains(LAZY_ALLOC) {
            lf.difference(LAZY_ALLOC).contains(flags)
        } else if lf.contains(COW) {
            lf.difference(COW)
                .union(PageTableFlags::WRITABLE)
                .contains(flags)
        } else {
            true
        }
    }

    fn translate_layer(&self, entry: &PageTableEntry) -> Option<Result<&PageTable, ()>> {
        match entry.frame() {
            Ok(next) => {
                let addr = to_higher_half(next.start_address()).as_ptr();
                Some(Ok(unsafe { &*addr }))
            }
            Err(FrameError::FrameNotPresent) => None,
            Err(FrameError::HugeFrame) => Some(Err(())),
        }
    }

    fn translate_addr(&self, v_addr: VirtAddr) -> Option<(PhysAddr, MappedFrame, PageTableFlags)> {
        let l4e = &self.l4[v_addr.p4_index()];
        let Ok(l3) = self.translate_layer(l4e)? else {
            unreachable!("512GiB maps are not supported");
        };

        let l3e = &l3[v_addr.p3_index()];
        let Ok(l2) = self.translate_layer(l3e)? else {
            let f = PhysFrame::from_start_address(l3e.addr()).unwrap();
            let addr = f.start_address() + (v_addr.as_u64() & 0o_777_777_7777);
            return Some((addr, MappedFrame::Size1GiB(f), l3e.flags()));
        };

        let l2e = &l2[v_addr.p2_index()];
        let Ok(l1) = self.translate_layer(l2e)? else {
            let f = PhysFrame::from_start_address(l2e.addr()).unwrap();
            let addr = f.start_address() + (v_addr.as_u64() & 0o_777_7777);
            return Some((addr, MappedFrame::Size2MiB(f), l2e.flags()));
        };

        let l1e = &l1[v_addr.p1_index()];
        match l1e.frame() {
            Ok(p) => p,
            Err(FrameError::FrameNotPresent) => return None,
            Err(FrameError::HugeFrame) => unreachable!("huge page at lvl 1"),
        };

        let f = PhysFrame::from_start_address(l1e.addr()).unwrap();
        let addr = f.start_address() + (v_addr.as_u64() & 0o_7777);
        Some((addr, MappedFrame::Size4KiB(f), l1e.flags()))
    }

    fn cr3(&self) -> PhysFrame {
        let virt = self.l4 as *const PageTable as u64;
        let phys = from_higher_half(VirtAddr::new(virt));
        PhysFrame::from_start_address(phys).unwrap()
    }

    fn is_active(&self) -> bool {
        Cr3::read().0 == self.cr3()
    }
}

//

impl PageMap {
    /// # Safety
    /// Unsafe if the page map was obtained with `PageMap::current`,
    /// the page table should have been owned by the bootloader if so.
    pub unsafe fn mark_owned(&mut self) {
        self.owned = true;
    }

    pub fn is_active(&self) -> bool {
        self.inner.read().is_active()
    }

    pub fn cr3(&self) -> PhysFrame {
        self.inner.read().cr3()
    }

    pub fn fork_into(&self, _into: &Self) {
        /* let mut from_offs = self.offs.write();
        let mut into_offs = new.offs.write();
        // TODO: CoW page tables also

        let hhdm_p4_index: usize = VirtAddr::new(hyperion_boot::hhdm_offset())
            .p4_index()
            .into();

        // TODO: iter maps instead of this mess
        let from_l4: &mut PageTable = from_offs.level_4_table();
        let into_l4: &mut PageTable = into_offs.level_4_table();

        for (from_l4e, into_l4e) in from_l4.iter_mut().zip(into_l4) {} */
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

        let inner = self.inner.read();

        println!("BEGIN PAGE TABLE ITER");
        let mut output = BTreeMap::new();
        let mut output_hh = BTreeMap::new();
        let l4 = Level4::from_pml4(inner.l4);
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

        let l4 = Level4::from_pml4(&self.inner.get_mut().l4);
        for (_, flags, entry) in l4.iter() {
            if !flags.contains(NO_FREE) {
                travel_level(entry);
            } else {
                hyperion_log::debug!("skip bit 9");
            }
        }

        let table = from_higher_half(VirtAddr::new(self.inner.get_mut().l4 as *const _ as u64));
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
pub enum TryMapError<T: PageSize> {
    Overflow,
    NotAligned,
    MapToError(MapToError<T>),
    WrongSize,
    AlreadyMapped,
    NotMapped,
}
