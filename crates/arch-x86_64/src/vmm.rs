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

#![allow(clippy::comparison_chain)]

use core::{
    arch::asm,
    mem::ManuallyDrop,
    ops::Range,
    sync::atomic::{fence, Ordering},
};

use hyperion_mem::{
    from_higher_half, is_higher_half,
    pmm::{self, PageFrame},
    to_higher_half,
    vmm::{Handled, MapTarget, MemoryInfo, NotHandled, PageFaultResult, PageMapImpl, Privilege},
};
use spin::{RwLock, RwLockWriteGuard};
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

//

pub const HIGHER_HALF_DIRECT_MAPPING: VirtAddr = VirtAddr::new_truncate(0xFFFF_8000_0000_0000);
pub const KERNEL_STACKS: VirtAddr = VirtAddr::new_truncate(0xFFFF_FFFD_8000_0000);
pub const KERNEL_EXECUTABLE: VirtAddr = VirtAddr::new_truncate(0xFFFF_FFFF_8000_0000);
pub const CURRENT_ADDRESS_SPACE: VirtAddr = VirtAddr::new_truncate(0xFFFF_FFFF_FFFF_F000);

/// the page should not be freed
pub const NO_FREE: PageTableFlags = PageTableFlags::BIT_9;
/// the page is shared and was originally writeable
pub const COW: PageTableFlags = PageTableFlags::BIT_10;
/// the page is not mapped and should not be mapped
pub const GUARD: PageTableFlags = PageTableFlags::BIT_11;
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

fn page_fault_1gib(
    info: &MemoryInfo,
    entry: &mut PageTableEntry,
    addr: VirtAddr,
) -> PageFaultResult {
    let mut flags = entry.flags();

    if flags.contains(COW) {
        todo!()
    } else if flags.contains(LAZY_ALLOC) {
        flags.remove(PageTableFlags::HUGE_PAGE);

        // convert the 1gib page into 2mib pages and allocate only one of them now

        let l3 = LockedPageMap::create_table(info, entry).unwrap();
        for l3e in l3.iter_mut() {
            l3e.set_flags(flags); // mark all with the original flags
        }
        let l3e = &mut l3[addr.p3_index()];

        return page_fault_2mib(info, l3e, addr);
    }

    Ok(NotHandled)
}

fn page_fault_2mib(
    info: &MemoryInfo,
    entry: &mut PageTableEntry,
    addr: VirtAddr,
) -> PageFaultResult {
    let mut flags = entry.flags();

    if flags.contains(COW) {
        todo!()
    } else if flags.contains(LAZY_ALLOC) {
        flags.remove(PageTableFlags::HUGE_PAGE);

        // convert the 2mib page into 4kib pages and allocate only one of them now

        let l2 = LockedPageMap::create_table(info, entry).unwrap();
        for l2e in l2.iter_mut() {
            l2e.set_flags(flags); // mark all with the original flags
        }
        let l2e = &mut l2[addr.p2_index()];

        return page_fault_4kib(info, l2e, addr);
    }

    Ok(NotHandled)
}

fn page_fault_4kib(
    info: &MemoryInfo,
    entry: &mut PageTableEntry,
    addr: VirtAddr,
) -> PageFaultResult {
    let mut flags = entry.flags();

    let new_frame = if flags.contains(COW) {
        // handle a fork CoW
        flags.remove(COW);
        flags.insert(PageTableFlags::WRITABLE);

        let page = Page::containing_address(addr);
        let old = entry.frame().unwrap();
        unsafe { pmm::PFA.fork_page_fault(old, page) }
    } else if flags.contains(LAZY_ALLOC) {
        // handle a lazy alloc
        flags.remove(LAZY_ALLOC);
        flags.insert(PageTableFlags::PRESENT);

        info.phys_pages.fetch_add(1, Ordering::Relaxed);
        PhysFrame::from_start_address(pmm::PFA.alloc(1).physical_addr()).unwrap()
    } else {
        return Ok(NotHandled);
    };

    entry.set_frame(new_frame, flags);
    MapperFlush::new(Page::<Size4KiB>::containing_address(addr)).flush();

    Err(Handled)
}

fn alloc_table(info: &MemoryInfo) -> PhysFrame {
    info.add_virt(1);
    info.add_phys(1);

    let frame = pmm::PFA.alloc(1);
    PhysFrame::<Size4KiB>::from_start_address(frame.physical_addr()).unwrap()
}

unsafe fn free_table(info: &MemoryInfo, f: PhysFrame) {
    unsafe { PageFrame::new(f.start_address(), 1) }.free();

    info.sub_virt(1);
    info.sub_phys(1);
}

//

// RwLock safe from lazy stack growing
struct SafeRwLock<T>(RwLock<T>);

impl<T> SafeRwLock<T> {
    fn new(t: T) -> Self {
        SafeRwLock(RwLock::new(t))
    }

    fn get_mut(&mut self) -> &mut T {
        self.0.get_mut()
    }

    fn write(&self) -> RwLockWriteGuard<T> {
        extern "C" fn stack_test() {
            let rsp: u64;
            unsafe { asm!("mov {rsp}, rsp", rsp = out(reg) rsp) };
            let rsp = (rsp / 8 * 8) as *const u8;

            // a minimum of 2 pages of free stack space should be more than enough
            unsafe { rsp.read_volatile() };
            unsafe { rsp.sub(0x1000).read_volatile() };
            unsafe { rsp.sub(0x2000).read_volatile() };
        }

        // FIXME: test only if the active page map is being modified
        stack_test();

        self.write_now()
    }

    fn write_now(&self) -> RwLockWriteGuard<T> {
        self.0.write()
    }

    fn read(&self) -> RwLockWriteGuard<T> {
        self.0.write()
    }
}

//

pub struct PageMap {
    inner: ManuallyDrop<SafeRwLock<LockedPageMap>>,
    owned: bool,
    info: MemoryInfo,
}

impl PageMapImpl for PageMap {
    fn page_fault(&self, v_addr: VirtAddr, privilege: Privilege) -> PageFaultResult {
        if privilege == Privilege::User && is_higher_half(v_addr.as_u64()) {
            // the user process shouldn't touch kernel memory anyways
            return Ok(NotHandled);
        }

        self.inner
            .write_now()
            .page_fault(&self.info, v_addr, privilege)
    }

    fn current() -> Self {
        // TODO: unsound, multiple mutable references to the same table could be made

        let (l4, _) = Cr3::read();
        let l4: &mut PageTable = unsafe { &mut *to_higher_half(l4.start_address()).as_mut_ptr() };

        Self {
            inner: ManuallyDrop::new(SafeRwLock::new(LockedPageMap { l4 })),
            info: MemoryInfo::symmetric(1),
            owned: false,
        }
    }

    fn new() -> Self {
        let info = MemoryInfo::zero();
        let cr3 = alloc_table(&info);
        let l4: &mut PageTable = unsafe { &mut *to_higher_half(cr3.start_address()).as_mut_ptr() };

        l4.zero();

        // TODO: Copy on write maps

        let page_map = Self {
            inner: ManuallyDrop::new(SafeRwLock::new(LockedPageMap { l4 })),
            info,
            owned: true,
        };

        // hyperion_log::debug!("higher half direct map");
        // TODO: pmm::PFA.allocations();
        assert_eq!(
            HIGHER_HALF_DIRECT_MAPPING.as_u64(),
            hyperion_boot::hhdm_offset()
        );
        page_map.map(
            HIGHER_HALF_DIRECT_MAPPING + 0x1000u64..HIGHER_HALF_DIRECT_MAPPING + 0x100000000u64,
            MapTarget::Borrowed(PhysAddr::new_truncate(0x1000)),
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
            MapTarget::Borrowed(PhysAddr::new(hyperion_boot::phys_addr() as _)),
            PageTableFlags::WRITABLE,
        );

        page_map
    }

    fn info(&self) -> &MemoryInfo {
        &self.info
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

                        let mut l0f = l1e.flags();
                        let target;
                        if l0f.contains(LAZY_ALLOC) {
                            target = MapTarget::LazyAlloc;
                        } else {
                            if l0f.contains(PageTableFlags::WRITABLE) {
                                // mark writeable pages as read only + CoW
                                l0f.remove(PageTableFlags::WRITABLE);
                                l0f.insert(COW);
                            }

                            // then fork the page
                            let mapped = Page::from_start_address(start).unwrap();
                            let new_frame = unsafe { pmm::PFA.fork(l0, mapped) }.start_address();
                            target = MapTarget::Preallocated(new_frame);
                        }

                        l1e.set_flags(l0f);
                        new.map(start..start + Size4KiB::SIZE, target, l0f);
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

    fn map(&self, v_addr: Range<VirtAddr>, p_addr: MapTarget, flags: PageTableFlags) {
        self.inner.write().map(&self.info, v_addr, p_addr, flags);
    }

    fn unmap(&self, v_addr: Range<VirtAddr>) {
        self.inner.write().unmap(&self.info, v_addr);
    }

    fn remap(&self, v_addr: Range<VirtAddr>, new_flags: PageTableFlags) {
        self.inner.write().remap(v_addr, new_flags);
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

    fn page_fault(
        &mut self,
        info: &MemoryInfo,
        v_addr: VirtAddr,
        _privilege: Privilege,
    ) -> PageFaultResult {
        // giant pages
        let l4e = &mut self.l4[v_addr.p4_index()];
        let Some(l3) = next_table(l4e) else {
            return Ok(NotHandled);
        };

        // huge pages
        let l3e = &mut l3[v_addr.p3_index()];
        let Some(l2) = next_table(l3e) else {
            return page_fault_1gib(info, l3e, v_addr);
        };

        // normal pages
        let l2e = &mut l2[v_addr.p2_index()];
        let Some(l1) = next_table(l2e) else {
            return page_fault_2mib(info, l2e, v_addr);
        };

        let l1e = &mut l1[v_addr.p1_index()];
        page_fault_4kib(info, l1e, v_addr)
    }

    fn map(
        &mut self,
        info: &MemoryInfo,
        Range { mut start, end }: Range<VirtAddr>,
        mut to: MapTarget,
        flags: PageTableFlags,
    ) {
        if !start.is_aligned(Size4KiB::SIZE)
            || !end.is_aligned(Size4KiB::SIZE)
            || !to.is_aligned(Size4KiB::SIZE)
        {
            panic!("Not aligned");
        }

        if flags.intersects(PageTableFlags::PRESENT | LAZY_ALLOC) {
            panic!("PRESENT and LAZY_ALLOC flags are not allowed, the VMM handles them");
        }
        hyperion_log::trace!("mapping [ 0x{start:016x}..0x{end:016x} ] to {to} with {flags:?}");

        loop {
            if start == end {
                break;
            } else if start > end {
                panic!("over-mapped");
            }

            let Err(err_1gib) = self.try_map_1gib(info, start..end, to, flags) else {
                // could crash if the last possible phys/virt page was mapped
                start += Size1GiB::SIZE;
                to.inc_addr(Size1GiB::SIZE);
                continue;
            };

            let Err(err_2mib) = self.try_map_2mib(info, start..end, to, flags) else {
                start += Size2MiB::SIZE;
                to.inc_addr(Size2MiB::SIZE);
                continue;
            };

            let Err(err_4kib) = self.try_map_4kib(info, start..end, to, flags) else {
                start += Size4KiB::SIZE;
                to.inc_addr(Size4KiB::SIZE);
                continue;
            };

            hyperion_log::error!("FIXME: failed to map [ 0x{start:016x} to {to} ]");
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

    fn is_phys_map_valid<S: PageSize>(to: MapTarget) -> Result<(), TryMapError<S>> {
        match to {
            MapTarget::Borrowed(to) | MapTarget::Preallocated(to) => {
                PhysFrame::<S>::from_start_address(to).map_err(|_| TryMapError::NotAligned)?;
            }
            MapTarget::LazyAlloc => {}
        }

        Ok(())
    }

    // None = HugeFrame
    fn create_table<'a>(
        info: &MemoryInfo,
        entry: &'a mut PageTableEntry,
    ) -> Option<&'a mut PageTable> {
        let flags =
            PageTableFlags::PRESENT | PageTableFlags::WRITABLE | PageTableFlags::USER_ACCESSIBLE;

        assert!(!entry.flags().intersects(LAZY_ALLOC | COW));
        let table = match entry.frame() {
            Ok(table) => table,
            Err(FrameError::FrameNotPresent) => {
                let table = alloc_table(info);
                entry.set_frame(table, flags);
                table
            }
            Err(FrameError::HugeFrame) => return None,
        };

        let addr = to_higher_half(table.start_address()).as_mut_ptr();
        Some(unsafe { &mut *addr })
    }

    fn try_map_if_diff<S: PageSize>(
        info: &MemoryInfo,
        entry: &mut PageTableEntry,
        to: MapTarget,
        flags: PageTableFlags,
    ) -> Result<(), TryMapError<S>> {
        let old: (PageTableFlags, PhysAddr) = (entry.flags(), entry.addr());

        let new: (PageTableFlags, PhysAddr) = match to {
            MapTarget::Borrowed(to) => (flags | PageTableFlags::PRESENT | NO_FREE, to),
            MapTarget::Preallocated(to) => (flags | PageTableFlags::PRESENT, to),
            MapTarget::LazyAlloc => (flags | LAZY_ALLOC, PhysAddr::new_truncate(0)),
        };

        if old == new {
            // already mapped but it is already correct
            return Ok(());
        }
        if !entry.is_unused() {
            return Err(TryMapError::AlreadyMapped);
        }

        let n_pages = S::SIZE as usize / 0x1000;
        info.add_virt(n_pages);
        if new.0.contains(NO_FREE) {
            debug_assert_ne!(new.1.as_u64(), 0);
        } else if new.0.contains(LAZY_ALLOC) {
            debug_assert_eq!(new.1.as_u64(), 0);
        } else if new.0.contains(COW) {
            debug_assert_ne!(new.1.as_u64(), 0);
            info.add_phys(n_pages);
        } else if new.0.contains(PageTableFlags::PRESENT) {
            debug_assert_ne!(new.1.as_u64(), 0);
            info.add_phys(n_pages);
        } else {
            todo!()
        }
        entry.set_addr(new.1, new.0);

        Ok(())
    }

    fn try_map_1gib(
        &mut self,
        info: &MemoryInfo,
        Range { start, end }: Range<VirtAddr>,
        to: MapTarget,
        flags: PageTableFlags,
    ) -> Result<(), TryMapError<Size1GiB>> {
        let from = Self::is_map_valid(start..end)?;
        Self::is_phys_map_valid(to)?;

        let Some(p3) = Self::create_table(info, &mut self.l4[from.p4_index()]) else {
            unreachable!("512GiB maps are not supported");
        };
        let p3e = &mut p3[from.p3_index()];

        Self::try_map_if_diff(info, p3e, to, flags | PageTableFlags::HUGE_PAGE)?;
        tlb::flush(from.start_address());

        Ok(())
    }

    fn try_map_2mib(
        &mut self,
        info: &MemoryInfo,
        Range { start, end }: Range<VirtAddr>,
        to: MapTarget,
        flags: PageTableFlags,
    ) -> Result<(), TryMapError<Size2MiB>> {
        let from = Self::is_map_valid(start..end)?;
        Self::is_phys_map_valid(to)?;

        let Some(p3) = Self::create_table(info, &mut self.l4[from.p4_index()]) else {
            unreachable!("512GiB maps are not supported");
        };
        let Some(p2) = Self::create_table(info, &mut p3[from.p3_index()]) else {
            return Err(TryMapError::WrongSize);
        };
        let p2e = &mut p2[from.p2_index()];

        Self::try_map_if_diff(info, p2e, to, flags | PageTableFlags::HUGE_PAGE)?;
        tlb::flush(from.start_address());

        Ok(())
    }

    fn try_map_4kib(
        &mut self,
        info: &MemoryInfo,
        Range { start, end }: Range<VirtAddr>,
        to: MapTarget,
        flags: PageTableFlags,
    ) -> Result<(), TryMapError<Size4KiB>> {
        let from = Self::is_map_valid(start..end)?;
        Self::is_phys_map_valid(to)?;

        let Some(p3) = Self::create_table(info, &mut self.l4[from.p4_index()]) else {
            unreachable!("512GiB maps are not supported");
        };
        let Some(p2) = Self::create_table(info, &mut p3[from.p3_index()]) else {
            return Err(TryMapError::WrongSize);
        };
        let Some(p1) = Self::create_table(info, &mut p2[from.p2_index()]) else {
            return Err(TryMapError::WrongSize);
        };
        let p1e = &mut p1[from.p1_index()];

        Self::try_map_if_diff(info, p1e, to, flags)?;
        tlb::flush(from.start_address());

        Ok(())
    }

    fn unmap(&mut self, info: &MemoryInfo, Range { mut start, end }: Range<VirtAddr>) {
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

            let Err(err_1gib) = self.try_unmap_1gib(info, start..end) else {
                // could crash if the last possible phys/virt page was mapped
                start += Size1GiB::SIZE;
                continue;
            };

            let Err(err_2mib) = self.try_unmap_2mib(info, start..end) else {
                start += Size2MiB::SIZE;
                continue;
            };

            let Err(err_4kib) = self.try_unmap_4kib(info, start..end) else {
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
        info: &MemoryInfo,
        entry: &mut PageTableEntry,
        should_be_huge_page: bool,
    ) -> Result<(), TryMapError<S>> {
        if entry.is_unused() {
            return Ok(());
        }

        let f = entry.flags();
        if f.contains(PageTableFlags::HUGE_PAGE) != should_be_huge_page {
            return Err(TryMapError::WrongSize);
        }

        let n_pages = S::SIZE as usize / 0x1000;
        let frames = unsafe { PageFrame::new(entry.addr(), n_pages) };
        if f.contains(NO_FREE) {
            debug_assert_ne!(entry.addr().as_u64(), 0);
            // obv don't free
        } else if f.contains(LAZY_ALLOC) {
            debug_assert_eq!(entry.addr().as_u64(), 0);
            // lazy allocs are not allocated yet so they cant be freed yet either
        } else if f.contains(COW) {
            debug_assert_ne!(entry.addr().as_u64(), 0);
            // the PMM handles double frees with CoW maps
            frames.free();
            info.sub_phys(n_pages);
        } else if f.contains(PageTableFlags::PRESENT) {
            debug_assert_ne!(entry.addr().as_u64(), 0);
            frames.free();
            info.sub_phys(n_pages);
        } else {
            todo!()
        }
        info.sub_virt(n_pages);
        entry.set_unused();

        Ok(())
    }

    fn try_unmap_1gib(
        &mut self,
        info: &MemoryInfo,
        Range { start, end }: Range<VirtAddr>,
    ) -> Result<(), TryMapError<Size1GiB>> {
        let from = Self::is_map_valid(start..end)?;

        let Some(p3) = Self::read_table(&mut self.l4[from.p4_index()])? else {
            unreachable!("512GiB maps are not supported");
        };
        let p3e = &mut p3[from.p3_index()];

        Self::try_unmap_if_correct_size(info, p3e, true)?;
        tlb::flush(from.start_address());

        Ok(())
    }

    fn try_unmap_2mib(
        &mut self,
        info: &MemoryInfo,
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

        Self::try_unmap_if_correct_size(info, p2e, true)?;
        tlb::flush(from.start_address());

        Ok(())
    }

    fn try_unmap_4kib(
        &mut self,
        info: &MemoryInfo,
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

        Self::try_unmap_if_correct_size(info, p1e, false)?;
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

            let l3 = match Self::translate_layer(l4) {
                Some(Ok(next)) => &next[start.p3_index()],
                Some(Err(())) => unreachable!(),
                None => return false,
            };
            if !self.is_mapped_layer(l3, contains) {
                return false;
            }

            let l2 = match Self::translate_layer(l3) {
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

            let l1 = match Self::translate_layer(l2) {
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

    fn translate_layer(entry: &PageTableEntry) -> Option<Result<&PageTable, ()>> {
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
        let Ok(l3) = Self::translate_layer(l4e)? else {
            unreachable!("512GiB maps are not supported");
        };

        let l3e = &l3[v_addr.p3_index()];
        let Ok(l2) = Self::translate_layer(l3e)? else {
            let f = PhysFrame::from_start_address(l3e.addr()).unwrap();
            let addr = f.start_address() + (v_addr.as_u64() & 0o7_777_777_777);
            return Some((addr, MappedFrame::Size1GiB(f), l3e.flags()));
        };

        let l2e = &l2[v_addr.p2_index()];
        let Ok(l1) = Self::translate_layer(l2e)? else {
            let f = PhysFrame::from_start_address(l2e.addr()).unwrap();
            let addr = f.start_address() + (v_addr.as_u64() & 0o7_777_777);
            return Some((addr, MappedFrame::Size2MiB(f), l2e.flags()));
        };

        let l1e = &l1[v_addr.p1_index()];
        match l1e.frame() {
            Ok(p) => p,
            Err(FrameError::FrameNotPresent) => return None,
            Err(FrameError::HugeFrame) => unreachable!("huge page at lvl 1"),
        };

        let f = PhysFrame::from_start_address(l1e.addr()).unwrap();
        let addr = f.start_address() + (v_addr.as_u64() & 0o7_777);
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

    fn clear(&mut self, info: &MemoryInfo) {
        Self::free_table(info, 4, self.l4);
    }

    fn free_table(info: &MemoryInfo, layer: u8, table: &mut PageTable) {
        for entry in table.iter_mut() {
            Self::free_entry(info, layer, entry);
        }
    }

    fn free_entry(info: &MemoryInfo, layer: u8, entry: &mut PageTableEntry) {
        if entry.is_unused() {
            return;
        }

        let f = entry.flags();

        let n_pages = match (layer, f.contains(PageTableFlags::HUGE_PAGE)) {
            (3, true) => 512 * 512,
            (2, true) => 512,
            (1, false) => 1,
            (2..=4, false) => {
                // next table
                let next = entry.frame().unwrap();
                let addr: *mut PageTable = to_higher_half(next.start_address()).as_mut_ptr();
                Self::free_table(info, layer - 1, unsafe { &mut *addr });

                unsafe { free_table(info, next) };
                entry.set_unused();
                return;
            }
            (_, _) => unreachable!(),
        };

        let frames = unsafe { PageFrame::new(entry.addr(), n_pages) };
        if f.contains(NO_FREE) {
            debug_assert_ne!(entry.addr().as_u64(), 0);
            // obv don't free
        } else if f.contains(LAZY_ALLOC) {
            debug_assert_eq!(entry.addr().as_u64(), 0);
            // lazy allocs are not allocated yet so they cant be freed yet either
        } else if f.contains(COW) {
            debug_assert_ne!(entry.addr().as_u64(), 0);
            // the PMM handles double frees with CoW maps
            frames.free();
            info.sub_phys(n_pages);
        } else if f.contains(PageTableFlags::PRESENT) {
            debug_assert_ne!(entry.addr().as_u64(), 0);
            frames.free();
            info.sub_phys(n_pages);
        } else {
            todo!()
        }
        info.sub_virt(n_pages);
        entry.set_unused();
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

    /* /// # Safety
    /// TODO: not safe
    pub unsafe fn unsafe_page_fault(
        &self,
        addr: VirtAddr,
        privilege: Privilege,
    ) -> PageFaultResult {
        // FIXME: page map entry locking
        self.inner.force_write_unlock();
    } */

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
}

impl Drop for PageMap {
    fn drop(&mut self) {
        if !self.owned {
            return;
        }

        let cr3 = {
            let mut inner = unsafe { ManuallyDrop::take(&mut self.inner) };
            let inner = inner.get_mut();
            inner.clear(&self.info);
            inner.cr3()
        };

        unsafe { free_table(&self.info, cr3) };

        fence(Ordering::SeqCst);
        let virt = self.info.virt_pages.load(Ordering::Relaxed);
        let phys = self.info.phys_pages.load(Ordering::Relaxed);

        if virt != 0 || phys != 0 {
            hyperion_log::error!("PageMap leaked memory, virt_pages={virt} phys_pages={phys}");
        }
    }
}

//

#[derive(Debug)]
pub enum TryMapError<T: PageSize> {
    Overflow,
    NotAligned,
    MapToError(MapToError<T>),
    WrongSize,
    AlreadyMapped,
    NotMapped,
}
