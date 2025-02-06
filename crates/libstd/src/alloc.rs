use core::{alloc::GlobalAlloc, ptr::NonNull, sync::atomic::AtomicUsize};

use hyperion_slab_alloc::{PageAlloc, Pages, SlabAllocator};
use hyperion_syscall::err::Error;

//

pub struct PageAllocator;

unsafe impl GlobalAlloc for PageAllocator {
    unsafe fn alloc(&self, layout: core::alloc::Layout) -> *mut u8 {
        let pages = layout.size().div_ceil(0x1000);

        let res: Result<Option<NonNull<u8>>, Error> = Err(Error::OUT_OF_VIRTUAL_MEMORY);
        // let res = palloc(pages);
        // println!("alloc syscall res: {res:?}");
        res.expect("page alloc").expect("null alloc").as_ptr()
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: core::alloc::Layout) {
        unimplemented!()
        // let pages = layout.size().div_ceil(0x1000);
        // assert!(pfree(NonNull::new(ptr).unwrap(), pages).is_ok());
    }
}

unsafe impl PageAlloc for PageAllocator {
    unsafe fn alloc(pages: usize) -> Pages {
        unimplemented!()
        // let alloc = palloc(pages).unwrap().unwrap();
        // unsafe { Pages::new(alloc.as_ptr(), pages) }
    }

    unsafe fn dealloc(frames: Pages) {
        unimplemented!()
        // pfree(NonNull::new(frames.as_ptr()).unwrap(), frames.len()).unwrap();
    }
}

//

pub type SlabAlloc = SlabAllocator<PageAllocator>;

#[global_allocator]
pub static GLOBAL_ALLOC: SlabAlloc = SlabAlloc::new();
// static GLOBAL_ALLOC: PageAlloc = PageAlloc;
