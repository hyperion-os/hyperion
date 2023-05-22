use x86_64::structures::paging::{FrameAllocator, PhysFrame, Size4KiB};

use crate::mem::pmm::PageFrameAllocator;

//

unsafe impl<'a> FrameAllocator<Size4KiB> for &'a PageFrameAllocator {
    fn allocate_frame(&mut self) -> Option<PhysFrame<Size4KiB>> {
        let f = self.alloc(1);

        PhysFrame::from_start_address(f.addr()).ok()
    }
}
