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
