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

use hyperion_log::println;
use hyperion_mem::{
    from_higher_half, is_higher_half,
    pmm::{self, PageFrame},
    to_higher_half,
    vmm::{NotHandled, PageFaultResult, PageMapImpl, Privilege},
};
use spin::RwLock;
use x86_64::{
    instructions::tlb,
    registers::control::{Cr3, Cr3Flags},
    structures::paging::{
        mapper::{MapToError, MappedFrame, MapperFlush, TranslateResult, UnmapError},
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

//

fn alloc_table() -> PhysFrame {
    let frame = pmm::PFA.alloc(1);
    PhysFrame::<Size4KiB>::from_start_address(frame.physical_addr()).unwrap()
}

fn free_table(f: PhysFrame) {
    let frame = unsafe { PageFrame::new(f.start_address(), 1) };
    pmm::PFA.free(frame)
}

//

pub struct PageMap {
    inner: RwLock<LockedPageMap>,
    owned: bool,
}

impl PageMapImpl for PageMap {
    fn page_fault(&self, _v_addr: VirtAddr, _privilege: Privilege) -> PageFaultResult {
        // TODO: lazy allocs

        Ok(NotHandled)
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

        // page_map.debug();

        page_map
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

    fn map(&self, v_addr: Range<VirtAddr>, p_addr: PhysAddr, flags: PageTableFlags) {
        self.inner.write().map(v_addr, p_addr, flags)
    }

    #[track_caller]
    fn unmap(&self, v_addr: Range<VirtAddr>) {
        hyperion_log::debug!("({})", core::panic::Location::caller());
        self.inner.write().unmap(v_addr)
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
        mut to: PhysAddr,
        flags: PageTableFlags,
    ) {
        if !start.is_aligned(Size4KiB::SIZE)
            || !end.is_aligned(Size4KiB::SIZE)
            || !to.is_aligned(Size4KiB::SIZE)
        {
            panic!("Not aligned");
        }

        hyperion_log::debug!(
            "mapping [ 0x{start:016x}..0x{end:016x} ] to 0x{to:016x} with {flags:?}"
        );

        loop {
            if start == end {
                break;
            } else if start > end {
                panic!("over-mapped");
            }

            let Err(err_1gib) = self.try_map_1gib(start..end, to, flags) else {
                // could crash if the last possible phys/virt page was mapped
                start += Size1GiB::SIZE;
                to += Size1GiB::SIZE;
                continue;
            };

            let Err(err_2mib) = self.try_map_2mib(start..end, to, flags) else {
                start += Size2MiB::SIZE;
                to += Size2MiB::SIZE;
                continue;
            };

            let Err(err_4kib) = self.try_map_4kib(start..end, to, flags) else {
                start += Size4KiB::SIZE;
                to += Size4KiB::SIZE;
                continue;
            };

            hyperion_log::error!("FIXME: failed to map [ 0x{start:016x} to 0x{to:016x} ]");
            hyperion_log::error!(" .. 1GiB: {err_1gib:?}");
            hyperion_log::error!(" .. 2MiB: {err_2mib:?}");
            hyperion_log::error!(" .. 4KiB: {err_4kib:?}");
        }
    }

    fn is_map_valid<S: PageSize>(
        Range { start, end }: Range<VirtAddr>,
        to: PhysAddr,
    ) -> Result<(Page<S>, PhysFrame<S>), TryMapError<S>> {
        let Ok(page) = Page::<S>::from_start_address(start) else {
            return Err(TryMapError::NotAligned);
        };

        let Some(limit) = end.as_u64().checked_sub(S::SIZE) else {
            hyperion_log::debug!("limit calc");
            return Err(TryMapError::Overflow);
        };

        if start.as_u64() > limit {
            hyperion_log::debug!("limit test {start:#018x}..{end:#018x}");
            return Err(TryMapError::Overflow);
        }

        let frame = PhysFrame::from_start_address(to).map_err(|_| TryMapError::NotAligned)?;

        Ok((page, frame))
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

    fn try_map_1gib(
        &mut self,
        Range { start, end }: Range<VirtAddr>,
        to: PhysAddr,
        flags: PageTableFlags,
    ) -> Result<(), TryMapError<Size1GiB>> {
        let (from, to) = Self::is_map_valid(start..end, to)?;

        let Some(p3) = Self::create_table(&mut self.l4[from.p4_index()]) else {
            unreachable!("512GiB maps are not supported");
        };
        let p3e = &mut p3[from.p3_index()];

        if !p3e.is_unused() {
            return Err(TryMapError::AlreadyMapped);
        }

        p3e.set_addr(
            to.start_address(),
            flags | PageTableFlags::HUGE_PAGE | PageTableFlags::PRESENT,
        );
        tlb::flush(from.start_address());

        Ok(())
    }

    fn try_map_2mib(
        &mut self,
        Range { start, end }: Range<VirtAddr>,
        to: PhysAddr,
        flags: PageTableFlags,
    ) -> Result<(), TryMapError<Size2MiB>> {
        let (from, to) = Self::is_map_valid(start..end, to)?;

        let Some(p3) = Self::create_table(&mut self.l4[from.p4_index()]) else {
            unreachable!("512GiB maps are not supported");
        };
        let Some(p2) = Self::create_table(&mut p3[from.p3_index()]) else {
            return Err(TryMapError::WrongSize);
        };
        let p2e = &mut p2[from.p2_index()];

        if !p2e.is_unused() {
            return Err(TryMapError::AlreadyMapped);
        }

        p2e.set_addr(
            to.start_address(),
            flags | PageTableFlags::HUGE_PAGE | PageTableFlags::PRESENT,
        );
        tlb::flush(from.start_address());

        Ok(())
    }

    fn try_map_4kib(
        &mut self,
        Range { start, end }: Range<VirtAddr>,
        to: PhysAddr,
        flags: PageTableFlags,
    ) -> Result<(), TryMapError<Size4KiB>> {
        let (from, to) = Self::is_map_valid(start..end, to)?;

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

        if !p1e.is_unused() {
            return Err(TryMapError::AlreadyMapped);
        }

        // hyperion_log::debug!("{:#018x}", to.start_address());
        p1e.set_frame(to, flags | PageTableFlags::PRESENT);
        tlb::flush(from.start_address());

        Ok(())
    }

    fn unmap(&mut self, Range { mut start, end }: Range<VirtAddr>) {
        if !start.is_aligned(Size4KiB::SIZE) || !end.is_aligned(Size4KiB::SIZE) {
            panic!("Not aligned");
        }

        hyperion_log::debug!("unmapping [ 0x{start:016x}..0x{end:016x} ]");

        loop {
            if start == end {
                break;
            } else if start > end {
                panic!("over-unmapped");
            }

            hyperion_log::debug!("unmapping {start:#018x}");

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

    fn try_unmap_1gib(
        &mut self,
        Range { start, end }: Range<VirtAddr>,
    ) -> Result<(), TryMapError<Size1GiB>> {
        let (from, _) = Self::is_map_valid(start..end, PhysAddr::new_truncate(0))?;

        let Some(p3) = Self::read_table(&mut self.l4[from.p4_index()])? else {
            unreachable!("512GiB maps are not supported");
        };
        let p3e = &mut p3[from.p3_index()];

        if p3e.is_unused() {
            hyperion_log::warn!("already unmapped");
            return Ok(());
        }

        if !p3e.flags().contains(PageTableFlags::HUGE_PAGE) {
            return Err(TryMapError::WrongSize);
        }

        // free_table(PhysFrame::from_start_address(p3e.addr()).unwrap());
        p3e.set_unused();
        tlb::flush(from.start_address());

        Ok(())
    }

    fn try_unmap_2mib(
        &mut self,
        Range { start, end }: Range<VirtAddr>,
    ) -> Result<(), TryMapError<Size2MiB>> {
        let (from, _) = Self::is_map_valid(start..end, PhysAddr::new_truncate(0))?;

        let Some(p3) = Self::read_table(&mut self.l4[from.p4_index()])? else {
            unreachable!("512GiB maps are not supported");
        };
        let Some(p2) = Self::read_table(&mut p3[from.p3_index()])? else {
            hyperion_log::warn!("already unmapped");
            return Ok(());
        };
        let p2e = &mut p2[from.p2_index()];

        if p2e.is_unused() {
            hyperion_log::warn!("already unmapped");
            return Ok(());
        }

        if !p2e.flags().contains(PageTableFlags::HUGE_PAGE) {
            return Err(TryMapError::WrongSize);
        }

        // free_table(PhysFrame::from_start_address(p2e.addr()).unwrap());
        p2e.set_unused();
        tlb::flush(from.start_address());

        Ok(())
    }

    fn try_unmap_4kib(
        &mut self,
        Range { start, end }: Range<VirtAddr>,
    ) -> Result<(), TryMapError<Size4KiB>> {
        let (from, _) = Self::is_map_valid(start..end, PhysAddr::new_truncate(0))?;

        let Some(p3) = Self::read_table(&mut self.l4[from.p4_index()])? else {
            hyperion_log::warn!("already unmapped p3");
            return Ok(());
        };
        let Some(p2) = Self::read_table(&mut p3[from.p3_index()])? else {
            hyperion_log::warn!("already unmapped p2");
            return Ok(());
        };
        let Some(p1) = Self::read_table(&mut p2[from.p2_index()])? else {
            hyperion_log::warn!("already unmapped p1");
            return Ok(());
        };
        let p1e = &mut p1[from.p1_index()];

        if p1e.is_unused() {
            hyperion_log::warn!("already unmapped");
            return Ok(());
        }

        if p1e.flags().contains(PageTableFlags::HUGE_PAGE) {
            panic!("4kib page cannot be a huge page");
        }

        // free_table(PhysFrame::from_start_address(p1e.addr()).unwrap());
        p1e.set_unused();
        tlb::flush(from.start_address());

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

            let Some((_, frame, flags)) = self.translate_addr(start) else {
                return false;
            };

            if !flags.contains(contains) {
                return false;
            }

            match frame {
                MappedFrame::Size4KiB(_) => start += Size4KiB::SIZE,
                MappedFrame::Size2MiB(_) => start += Size2MiB::SIZE,
                MappedFrame::Size1GiB(_) => start += Size1GiB::SIZE,
            };
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
        let phys = match l1e.frame() {
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
                        if !flags.contains(PageTableFlags::BIT_9) {
                            travel_level(entry);
                        }
                    }

                    let table = from_higher_half(VirtAddr::new(l3.0 as *const _ as u64));
                    Pfa.deallocate_frame(PhysFrame::containing_address(table));
                }
                WalkTableIterResult::Level2(l2) => {
                    for (_, flags, entry) in l2.iter() {
                        if !flags.contains(PageTableFlags::BIT_9) {
                            travel_level(entry);
                        }
                    }

                    let table = from_higher_half(VirtAddr::new(l2.0 as *const _ as u64));
                    Pfa.deallocate_frame(PhysFrame::containing_address(table));
                }
                WalkTableIterResult::Level1(l1) => {
                    for (_, flags, entry) in l1.iter() {
                        if !flags.contains(PageTableFlags::BIT_9) {
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
            if !flags.contains(PageTableFlags::BIT_9) {
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
}

#[derive(Debug)]
pub enum TryUnmapError<T: PageSize> {
    Overflow,
    NotAligned,
    MapToError(MapToError<T>),
    WrongSize,
    AlreadyMapped,
}

fn try_map_sized<T>(
    table: &mut OffsetPageTable,
    start: VirtAddr,
    end: VirtAddr,
    p_addr: PhysAddr,
    flags: PageTableFlags,
) -> Result<(), TryMapError<T>>
where
    T: PageSize,
    for<'a> OffsetPageTable<'a>: Mapper<T>,
{
    let Some(mapping_end) = start.as_u64().checked_add(T::SIZE - 1) else {
        return Err(TryMapError::Overflow);
    };

    if mapping_end > end.as_u64() {
        return Err(TryMapError::Overflow);
    }

    if !start.is_aligned(T::SIZE) || !p_addr.is_aligned(T::SIZE) {
        return Err(TryMapError::NotAligned);
    }

    let page = Page::<T>::containing_address(start);
    let frame = PhysFrame::<T>::containing_address(p_addr);

    let result = unsafe { table.map_to(page, frame, flags, &mut Pfa) };

    if let Err(MapToError::PageAlreadyMapped(old_frame)) = result {
        if old_frame == frame {
            return Ok(());
        }
    }

    result.map_err(|err| TryMapError::MapToError(err))?.flush();

    /* hyperion_log::debug!("mapped 1GiB at 0x{:016x}", start);
    crash_after_nth(10); */

    Ok(())
}

fn try_unmap_sized<T>(
    table: &mut OffsetPageTable,
    start: VirtAddr,
    _end: VirtAddr,
) -> Result<(), TryMapError<T>>
where
    T: PageSize,
    for<'a> OffsetPageTable<'a>: Mapper<T>,
{
    let Some(_mapping_end) = start.as_u64().checked_add(T::SIZE - 1) else {
        return Err(TryMapError::Overflow);
    };

    /* if mapping_end > end.as_u64() {
        return Err(TryMapSizedError::Overflow);
    } */

    if !start.is_aligned(T::SIZE) {
        return Err(TryMapError::NotAligned);
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
        Err(UnmapError::ParentEntryHugePage) => Err(TryMapError::WrongSize),
        Err(_err) => panic!("{_err:?}"),
    }
}

fn v_addr_checked_add(addr: VirtAddr, rhs: u64) -> Option<VirtAddr> {
    VirtAddr::try_new(addr.as_u64().checked_add(rhs)?).ok()
}

fn p_addr_checked_add(addr: PhysAddr, rhs: u64) -> Option<PhysAddr> {
    PhysAddr::try_new(addr.as_u64().checked_add(rhs)?).ok()
}
