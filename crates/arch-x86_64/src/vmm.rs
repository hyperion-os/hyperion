use alloc::collections::BTreeMap;
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

        hyperion_log::trace!("HHDM: 0x{:016x}", hyperion_boot::hhdm_offset());

        // TODO: Copy on write maps
        // lower map, unused
        /* page_map.map(
            VirtAddr::new(0x1000)..VirtAddr::new(0x100000000),
            PhysAddr::new(0x1000),
            PageTableFlags::WRITABLE,
        );
        page_map.map(
            VirtAddr::new(0xfd00000000)..VirtAddr::new(0x10000000000),
            PhysAddr::new(0xfd00000000),
            PageTableFlags::WRITABLE,
        ); */
        // higher half map
        page_map.map(
            VirtAddr::new(hyperion_boot::hhdm_offset())
                ..VirtAddr::new(0x10000000000 + hyperion_boot::hhdm_offset()),
            PhysAddr::new(0x0),
            PageTableFlags::WRITABLE,
        );
        // kernel map
        page_map.map(
            VirtAddr::new(hyperion_boot::virt_addr() as _)..VirtAddr::new(u64::MAX),
            PhysAddr::new(hyperion_boot::phys_addr() as _),
            PageTableFlags::WRITABLE,
        );

        page_map
    }

    fn activate(&self) {
        let mut offs = self.offs.write();

        let virt = offs.level_4_table() as *mut PageTable as *const () as u64;
        let phys = from_higher_half(VirtAddr::new(virt));

        hyperion_log::trace!("Switching page maps");

        unsafe { Cr3::write(PhysFrame::containing_address(phys), Cr3Flags::empty()) };
    }

    fn virt_to_phys(&self, addr: VirtAddr) -> Option<PhysAddr> {
        self.offs.read().translate_addr(addr)
    }

    fn phys_to_virt(&self, addr: PhysAddr) -> VirtAddr {
        to_higher_half(addr)
    }

    fn map(&self, v_addr: Range<VirtAddr>, mut p_addr: PhysAddr, mut flags: PageTableFlags) {
        if !v_addr.start.is_aligned(Size4KiB::SIZE) || !p_addr.is_aligned(Size4KiB::SIZE) {
            panic!("Not aligned");
        }

        let mut table = self.offs.write();
        let mut pmm = Pfa(pmm::PageFrameAllocator::get());
        let table = &mut table; // to make the formatting nicer
        let pmm = &mut pmm;

        flags.insert(PageTableFlags::PRESENT);
        // TODO: remove
        flags.insert(PageTableFlags::USER_ACCESSIBLE);
        /* let flags =
        PageTableFlags::PRESENT | PageTableFlags::WRITABLE | PageTableFlags::USER_ACCESSIBLE; */

        let Range { mut start, end } = v_addr;

        loop {
            let size;

            if Self::try_map_sized::<Size1GiB>(table, start, end, p_addr, flags, pmm) {
                size = Size1GiB::SIZE;
            } else if Self::try_map_sized::<Size2MiB>(table, start, end, p_addr, flags, pmm) {
                size = Size2MiB::SIZE;
            } else if Self::try_map_sized::<Size4KiB>(table, start, end, p_addr, flags, pmm) {
                size = Size4KiB::SIZE;
            } else {
                hyperion_log::error!("FIXME: failed to map [ 0x{start:016x} to 0x{p_addr:016x} ]");
                size = Size4KiB::SIZE;
            }

            fn v_addr_checked_add(addr: VirtAddr, rhs: u64) -> Option<VirtAddr> {
                VirtAddr::try_new(addr.as_u64().checked_add(rhs)?).ok()
            }

            fn p_addr_checked_add(addr: PhysAddr, rhs: u64) -> Option<PhysAddr> {
                PhysAddr::try_new(addr.as_u64().checked_add(rhs)?).ok()
            }

            let (Some(next_start), Some(next_p_addr)) = (
                v_addr_checked_add(start, size),
                p_addr_checked_add(p_addr, size),
            ) else {
                return;
            };
            start = next_start;
            p_addr = next_p_addr;

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

    fn unmap(&self, _v_addr: Range<VirtAddr>) {
        let mut _table = self.offs.write();

        todo!()
        /* let mut offs: u64 = 0;

        while v_addr.start + offs <= v_addr.end {
            let page = Page::<Size4KiB>::containing_address(v_addr.start + offs);
            if let Ok(ok) = table.unmap(page) {
                ok.1.flush();
                continue;
            }

            let page = Page::<Size2MiB>::containing_address(v_addr.start + offs);
            if let Ok(ok) = table.unmap(page) {
                ok.1.flush();
                continue;
            }

            let page = Page::<Size1GiB>::containing_address(v_addr.start + offs);
            if let Ok(ok) = table.unmap(page) {
                ok.1.flush();
                continue;
            }

            panic!("Failed to unmap");
        }

        if v_addr.start + offs > v_addr.end {
            hyperion_log::error!("FIXME: over-unmapped");
        } */
    }
}

impl PageMap {
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

        if mapping_end <= end.as_u64() && start.is_aligned(T::SIZE) && p_addr.is_aligned(T::SIZE) {
            let page = Page::<T>::containing_address(start);
            let frame = PhysFrame::<T>::containing_address(p_addr);
            if let Ok(ok) = unsafe { table.map_to(page, frame, flags, pmm) } {
                /* hyperion_log::debug!("mapped 1GiB at 0x{:016x}", start);
                crash_after_nth(10); */
                ok.flush();

                return true;
            }
        }

        false
    }

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
        let l4 = Level4::from_pml4(offs.level_4_table());
        for (i, flags, entry) in l4.iter() {
            if i < hhdm_p4_index {
                continue;
            }
            travel_level(flags, entry, &mut output);
        }
        println!("END PAGE TABLE ITER");

        /* println!("BEGIN PAGE TABLE ITER");
        for (&segment_start, &segment_size) in output.iter() {
            let segment_end = segment_start + segment_size;
            println!("PAGING SEGMENT [ 0x{segment_start:016x}..0x{segment_end:016x} ]");

            crash_after_nth(10);
        }
        println!("END PAGE TABLE ITER"); */

        println!("BEGIN PAGE TABLE SEGMENTS");
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
        println!("END PAGE TABLE SEGMENTS");
    }
}
