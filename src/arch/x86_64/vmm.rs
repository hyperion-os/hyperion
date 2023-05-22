use spin::RwLock;
use x86_64::{
    registers::control::Cr3,
    structures::paging::{
        Mapper, OffsetPageTable, Page, PageTable, PageTableFlags, PhysFrame, Size4KiB, Translate,
    },
    PhysAddr, VirtAddr,
};

use crate::{
    boot::hhdm_offset,
    mem::{pmm, to_higher_half, vmm::PageMapImpl},
};

//

pub struct PageMap {
    offs: RwLock<OffsetPageTable<'static>>,
}

//

impl PageMapImpl for PageMap {
    fn init() -> Self {
        let (l4, _) = Cr3::read();

        let virt = to_higher_half(l4.start_address());
        let table: *mut PageTable = virt.as_mut_ptr();
        let table = unsafe { &mut *table };

        let offs = unsafe { OffsetPageTable::new(table, VirtAddr::new(hhdm_offset())) };

        Self {
            offs: RwLock::new(offs),
        }
    }

    fn virt_to_phys(&self, addr: VirtAddr) -> Option<PhysAddr> {
        self.offs.read().translate_addr(addr)
    }

    fn phys_to_virt(&self, addr: PhysAddr) -> VirtAddr {
        to_higher_half(addr)
    }

    fn map(&self, v_addr: VirtAddr, p_addr: PhysAddr, pages: usize) {
        let mut offs = self.offs.write();
        let mut pmm = pmm::PageFrameAllocator::get();

        let flags = PageTableFlags::PRESENT | PageTableFlags::WRITABLE;

        for i in (0..pages).map(|i| i * 4096) {
            let page = Page::<Size4KiB>::containing_address(v_addr + i);
            let frame = PhysFrame::containing_address(p_addr + i);

            unsafe {
                offs.map_to(page, frame, flags, &mut pmm).unwrap().flush();
            }
        }
    }

    fn unmap(&self, v_addr: VirtAddr, pages: usize) {
        let mut offs = self.offs.write();

        for i in (0..pages).map(|i| i * 4096) {
            let page = Page::<Size4KiB>::from_start_address(v_addr + i).unwrap();
            offs.unmap(page).unwrap().1.flush();
        }
    }
}
