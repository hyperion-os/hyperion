use core::ops::Range;

use x86_64::{structures::paging::PageTableFlags, PhysAddr, VirtAddr};

//

pub trait PageMapImpl {
    fn current() -> Self;

    fn new() -> Self;

    fn activate(&self);

    fn virt_to_phys(&self, v_addr: VirtAddr) -> Option<PhysAddr>;
    fn phys_to_virt(&self, p_addr: PhysAddr) -> VirtAddr;

    fn map(&self, v_addr: Range<VirtAddr>, p_addr: PhysAddr, flags: PageTableFlags);
    fn unmap(&self, v_addr: Range<VirtAddr>);
}

//

/* #[cfg(test)]
mod tests {
    /* use x86_64::VirtAddr;

    use super::{PageMap, PageMapImpl};
    use crate::mem::pmm::PageFrameAllocator;

    #[test_case]
    fn two_virt_to_one_phys() {
        let map = PageMap::init();
        let frame = PageFrameAllocator::get().alloc(1);
        map.unmap(VirtAddr::new(0x1000), 1);
        map.map(VirtAddr::new(0x0), frame.physical_addr(), 1);
        map.map(VirtAddr::new(0x1000), frame.physical_addr(), 1);

        let a1 = unsafe { &mut *(0x1 as *mut u8) };
        let a2 = unsafe { &mut *(0x1001 as *mut u8) };

        *a1 = 50;
        assert_eq!(a1, a2);
        *a1 = 150;
        assert_eq!(a1, a2);
    } */
} */
