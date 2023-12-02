use core::{alloc::GlobalAlloc, ptr::NonNull};

pub use core_alloc::*;
use hyperion_slab_alloc::{PageFrameAllocator, PageFrames, SlabAllocator};
use hyperion_syscall::{palloc, pfree};

//

pub struct PageAlloc;

unsafe impl GlobalAlloc for PageAlloc {
    unsafe fn alloc(&self, layout: core::alloc::Layout) -> *mut u8 {
        let pages = layout.size().div_ceil(0x1000);

        let res = palloc(pages);
        // println!("alloc syscall res: {res:?}");
        res.expect("page alloc").expect("null alloc").as_ptr()
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: core::alloc::Layout) {
        let pages = layout.size().div_ceil(0x1000);
        assert!(pfree(NonNull::new(ptr).unwrap(), pages).is_ok());
    }
}

impl PageFrameAllocator for PageAlloc {
    fn alloc(pages: usize) -> PageFrames {
        let alloc = palloc(pages).unwrap().unwrap();
        unsafe { PageFrames::new(alloc.as_ptr(), pages) }
    }

    fn free(frames: PageFrames) {
        pfree(NonNull::new(frames.as_ptr()).unwrap(), frames.len()).unwrap();
    }
}

//

pub type SlabAlloc = SlabAllocator<PageAlloc, spin::Mutex<()>>;

#[global_allocator]
static GLOBAL_ALLOC: SlabAlloc = SlabAlloc::new();
// static GLOBAL_ALLOC: PageAlloc = PageAlloc;
