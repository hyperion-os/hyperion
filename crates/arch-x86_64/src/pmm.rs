use hyperion_mem::pmm::{self, PageFrame};
use x86_64::structures::paging::{FrameAllocator, PhysFrame, Size4KiB};

//

pub struct Pfa;

//

impl Pfa {
    pub fn deallocate_frame(&mut self, frame: PhysFrame<Size4KiB>) {
        hyperion_log::debug!("dealloc 1kib");
        let frame = unsafe { PageFrame::new(frame.start_address(), 1) };
        pmm::PFA.free(frame);
    }
}

unsafe impl FrameAllocator<Size4KiB> for Pfa {
    fn allocate_frame(&mut self) -> Option<PhysFrame<Size4KiB>> {
        let f = pmm::PFA.alloc(1);

        PhysFrame::from_start_address(f.physical_addr()).ok()
    }
}
