use alloc::{collections::BTreeMap, sync::Arc};
use core::{cmp::Ordering, ops::Range};

use hyperion_log::println;
use hyperion_mem::{
    from_higher_half,
    pmm::{self, PageFrameAllocator},
    to_higher_half,
    vmm::PageMapImpl,
};
use spin::{Mutex, RwLock};
use x86_64::{
    registers::control::{Cr3, Cr3Flags},
    structures::paging::{
        mapper::{MappedFrame, TranslateResult, UnmapError},
        Mapper, OffsetPageTable, Page, PageSize, PageTable, PageTableFlags, PhysFrame, Size1GiB,
        Size2MiB, Size4KiB, Translate,
    },
    PhysAddr, VirtAddr,
};

use super::pmm::Pfa;
use crate::paging::{Level4, WalkTableIterResult};

//

pub struct PageMap {
    offs: RwLock<OffsetPageTable<'static>>,
}

// TODO: drop

//

fn _crash_after_nth(nth: usize) {
    static TABLE: Mutex<BTreeMap<usize, usize>> = Mutex::new(BTreeMap::new());
    let mut table = TABLE.lock();

    let left = table.entry(nth).or_insert_with(|| nth);
    *left -= 1;

    if *left == 0 {
        panic!("crash_after_nth {nth} complete");
    }
}

impl PageMapImpl for PageMap {
    fn current() -> Self {
        // TODO: unsound, multiple mutable references to the same table could be made

        let (l4, _) = Cr3::read();
        let virt = to_higher_half(l4.start_address());
        let table: *mut PageTable = virt.as_mut_ptr();
        let table = unsafe { &mut *table };

        let offs =
            unsafe { OffsetPageTable::new(table, VirtAddr::new(hyperion_boot::hhdm_offset())) };
        let offs = RwLock::new(offs);

        Self { offs }
    }

    fn new() -> Self {
        let frame = PageFrameAllocator::get().alloc(1);
        let new_table: &mut PageTable = unsafe { &mut *frame.virtual_addr().as_mut_ptr() };

        new_table.zero();

        let offs =
            unsafe { OffsetPageTable::new(new_table, VirtAddr::new(hyperion_boot::hhdm_offset())) };
        let offs = RwLock::new(offs);

        let page_map = Self { offs };

        hyperion_log::debug!("null ptr guard unmap");
        page_map.unmap(VirtAddr::new(0x0000)..VirtAddr::new(0x1000));

        // TODO: Copy on write maps

        hyperion_log::debug!("higher half direct map");
        let hhdm = VirtAddr::new(hyperion_boot::hhdm_offset());
        page_map.map(
            hhdm..hhdm + 0x10000000000u64,
            PhysAddr::new(0x0),
            PageTableFlags::PRESENT | PageTableFlags::WRITABLE,
        );

        // TODO: less dumb kernel mapping
        hyperion_log::debug!("kernel map");
        let kernel = VirtAddr::new(hyperion_boot::virt_addr() as _);
        page_map.map(
            kernel..VirtAddr::new(u64::MAX),
            PhysAddr::new(hyperion_boot::phys_addr() as _),
            PageTableFlags::PRESENT | PageTableFlags::WRITABLE,
        );

        page_map
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
        let mut pmm = Pfa(pmm::PageFrameAllocator::get());
        let table = &mut table; // to make the formatting nicer
        let pmm = &mut pmm;

        let Range { mut start, end } = v_addr;
        let mut size;

        hyperion_log::debug!(
            "mapping [ 0x{start:016x}..0x{end:016x} ] to 0x{p_addr:016x} with {flags:?}"
        );

        loop {
            if try_map_sized::<Size1GiB>(table, start, end, p_addr, flags, pmm) {
                size = Size1GiB::SIZE;
            } else if try_map_sized::<Size2MiB>(table, start, end, p_addr, flags, pmm) {
                size = Size2MiB::SIZE;
            } else if try_map_sized::<Size4KiB>(table, start, end, p_addr, flags, pmm) {
                size = Size4KiB::SIZE;
            } else {
                hyperion_log::error!("FIXME: failed to map [ 0x{start:016x} to 0x{p_addr:016x} ]");
                size = Size4KiB::SIZE;
            }

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

        hyperion_log::debug!("unmapping [ 0x{start:016x}..0x{end:016x} ]");

        loop {
            // hyperion_log::debug!("unmapping {start:?}..{end:?}");

            if try_unmap_sized::<Size1GiB>(table, start, end) {
                size = Size1GiB::SIZE;
            } else if try_unmap_sized::<Size2MiB>(table, start, end) {
                size = Size2MiB::SIZE;
            } else if try_unmap_sized::<Size4KiB>(table, start, end) {
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
    pub fn debug(&self) {
        fn travel_level(
            flags: PageTableFlags,
            l: WalkTableIterResult,
            output: &mut BTreeMap<u64, (PageTableFlags, u64)>,
        ) {
            match l {
                WalkTableIterResult::Size1GiB(addr) => {
                    output.insert(addr.as_u64(), (flags, Size1GiB::SIZE));
                }
                WalkTableIterResult::Size2MiB(addr) => {
                    output.insert(addr.as_u64(), (flags, Size2MiB::SIZE));
                }
                WalkTableIterResult::Size4KiB(addr) => {
                    output.insert(addr.as_u64(), (flags, Size4KiB::SIZE));
                }
                WalkTableIterResult::Level3(l3) => {
                    for (_, flags, entry) in l3.iter() {
                        travel_level(flags, entry, output);
                    }
                }
                WalkTableIterResult::Level2(l2) => {
                    for (_, flags, entry) in l2.iter() {
                        travel_level(flags, entry, output);
                    }
                }
                WalkTableIterResult::Level1(l1) => {
                    for (_, flags, entry) in l1.iter() {
                        travel_level(flags, entry, output);
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
            if i < hhdm_p4_index {
                travel_level(flags, entry, &mut output);
            } else {
                travel_level(flags, entry, &mut output_hh);
            }
        }
        println!("END PAGE TABLE ITER");

        /* println!("BEGIN PAGE TABLE ITER");
        for (&segment_start, &segment_size) in output.iter() {
            let segment_end = segment_start + segment_size;
            println!("PAGING SEGMENT [ 0x{segment_start:016x}..0x{segment_end:016x} ]");

            crash_after_nth(10);
        }
        println!("END PAGE TABLE ITER"); */

        let print_output = |output: BTreeMap<u64, (PageTableFlags, u64)>| {
            let mut last = None;
            for (segment_start, (mut flags, segment_size)) in output {
                let segment_end = segment_start + segment_size;
                flags.remove(PageTableFlags::ACCESSED);
                flags.remove(PageTableFlags::DIRTY);
                flags.remove(PageTableFlags::HUGE_PAGE);

                /* if segment_start == 0x000000fd00200000 {
                    crash_after_nth(2);
                } */

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
        println!("BEGIN HIGER HALF PAGE TABLE SEGMENTS");
        print_output(output_hh);
        println!("END PAGE TABLE SEGMENTS");
    }
}

fn try_map_sized<T>(
    table: &mut OffsetPageTable,
    start: VirtAddr,
    end: VirtAddr,
    p_addr: PhysAddr,
    flags: PageTableFlags,
    pmm: &mut Pfa,
) -> bool
where
    T: PageSize,
    for<'a> OffsetPageTable<'a>: Mapper<T>,
{
    let Some(mapping_end) = start.as_u64().checked_add(T::SIZE - 1) else {
        return false;
    };

    if !(mapping_end <= end.as_u64() && start.is_aligned(T::SIZE) && p_addr.is_aligned(T::SIZE)) {
        return false;
    }

    let page = Page::<T>::containing_address(start);
    let frame = PhysFrame::<T>::containing_address(p_addr);

    if let Ok(ok) = unsafe { table.map_to(page, frame, flags, pmm) } {
        /* hyperion_log::debug!("mapped 1GiB at 0x{:016x}", start);
        crash_after_nth(10); */
        ok.flush();

        return true;
    }

    false
}

fn try_unmap_sized<T>(table: &mut OffsetPageTable, start: VirtAddr, end: VirtAddr) -> bool
where
    T: PageSize,
    for<'a> OffsetPageTable<'a>: Mapper<T>,
{
    let Some(mapping_end) = start.as_u64().checked_add(T::SIZE - 1) else {
        return false;
    };

    if !(mapping_end <= end.as_u64() && start.is_aligned(T::SIZE)) {
        return false;
    }

    let page = Page::<T>::containing_address(start);

    match table.unmap(page) {
        Ok((_, ok)) => {
            ok.flush();
            true
        }
        Err(UnmapError::PageNotMapped) => {
            // hyperion_log::debug!("already not mapped");
            true
        }
        Err(_err) => {
            // hyperion_log::error!("{err:?}");
            false
        }
    }
}

fn v_addr_checked_add(addr: VirtAddr, rhs: u64) -> Option<VirtAddr> {
    VirtAddr::try_new(addr.as_u64().checked_add(rhs)?).ok()
}

fn p_addr_checked_add(addr: PhysAddr, rhs: u64) -> Option<PhysAddr> {
    PhysAddr::try_new(addr.as_u64().checked_add(rhs)?).ok()
}
