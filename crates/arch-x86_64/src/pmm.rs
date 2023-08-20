use hyperion_mem::pmm;
use x86_64::structures::paging::{FrameAllocator, PhysFrame, Size4KiB};

//

pub struct Pfa;

//

unsafe impl FrameAllocator<Size4KiB> for Pfa {
    fn allocate_frame(&mut self) -> Option<PhysFrame<Size4KiB>> {
        let f = pmm::PFA.alloc(1);

        PhysFrame::from_start_address(f.physical_addr()).ok()
    }
}
