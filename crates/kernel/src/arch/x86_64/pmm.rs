use hyperion_mem::pmm::PageFrameAllocator;
use x86_64::structures::paging::{FrameAllocator, PhysFrame, Size4KiB};

//

pub struct Pfa<'a>(pub &'a PageFrameAllocator);

//

unsafe impl FrameAllocator<Size4KiB> for Pfa<'_> {
    fn allocate_frame(&mut self) -> Option<PhysFrame<Size4KiB>> {
        let f = self.0.alloc(1);

        PhysFrame::from_start_address(f.physical_addr()).ok()
    }
}
