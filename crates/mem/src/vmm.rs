use core::ops::Range;

use x86_64::{structures::paging::PageTableFlags, PhysAddr, VirtAddr};

//

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Privilege {
    User,
    Kernel,
}

/// inversed to make `?` more useful
///
/// TODO: impl try
pub type PageFaultResult = Result<NotHandled, Handled>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Handled;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NotHandled;

//

pub trait PageMapImpl {
    /// handle a page fault, possibly related to lazy mapping or CoW pages
    fn page_fault(&self, v_addr: VirtAddr, privilege: Privilege) -> PageFaultResult;

    /// take the current virtual address space
    fn current() -> Self;

    /// create a new virtual address space
    fn new() -> Self;

    /// lazy clone this virtual address space
    fn fork(&self) -> Self;

    /// switch to this virtual address space
    fn activate(&self);

    /// convert virtual addr to physical addr, by reading the page tables
    fn virt_to_phys(&self, v_addr: VirtAddr) -> Option<PhysAddr>;

    /// convert physical addr to virtual addr, by moving it to the higher half
    fn phys_to_virt(&self, p_addr: PhysAddr) -> VirtAddr;

    /// map physical memory into virtual memory
    ///
    /// `p_addr` None means that the pages need to be allocated (possibly lazily on use)
    fn map(&self, v_addr: Range<VirtAddr>, p_addr: Option<PhysAddr>, flags: PageTableFlags);

    /// unmap a range of virtual memory
    fn unmap(&self, v_addr: Range<VirtAddr>);

    /// test if a virtual memory range is mapped with (at least) the given flags
    fn is_mapped(&self, v_addr: Range<VirtAddr>, has_at_least: PageTableFlags) -> bool;
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
