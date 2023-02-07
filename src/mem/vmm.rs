pub use crate::arch::vmm::PageMap;
use x86_64::{PhysAddr, VirtAddr};

//

pub trait PageMapImpl {
    fn init() -> Self;

    fn virt_to_phys(&self, v_addr: VirtAddr) -> Option<PhysAddr>;
    fn phys_to_virt(&self, p_addr: PhysAddr) -> VirtAddr;

    fn map(&self, v_addr: VirtAddr, p_addr: PhysAddr, pages: usize);
    fn unmap(&self, v_addr: VirtAddr, pages: usize);
}

//

#[cfg(test)]
mod tests {
    use super::{PageMap, PageMapImpl};
    use crate::mem::pmm::PageFrameAllocator;
    use x86_64::VirtAddr;

    #[test_case]
    fn two_virt_to_one_phys() {
        let map = PageMap::init();
        let frame = PageFrameAllocator::get().alloc(1);
        map.unmap(VirtAddr::new(0x1000), 1);
        map.map(VirtAddr::new(0x0), frame.addr(), 1);
        map.map(VirtAddr::new(0x1000), frame.addr(), 1);

        let a1 = unsafe { &mut *(0x1 as *mut u8) };
        let a2 = unsafe { &mut *(0x1001 as *mut u8) };

        *a1 = 50;
        assert_eq!(a1, a2);
        *a1 = 150;
        assert_eq!(a1, a2);
    }
}
