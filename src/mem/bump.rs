use super::{
    pmm::{PageFrame, PageFrameAllocator},
    to_higher_half,
};
use crate::error;
use core::{
    alloc::{GlobalAlloc, Layout},
    ptr::null_mut,
};
use spin::{Lazy, Mutex};
use x86_64::{align_up, PhysAddr};

//

#[global_allocator]
static ALLOC: BumpAllocator = BumpAllocator {
    inner: Lazy::new(|| {
        let pages = PageFrameAllocator::get().alloc(4);
        BumpAllocInner {
            marker: Mutex::new(pages.addr()),
            pages,
        }
    }),
};

//

struct BumpAllocator {
    inner: Lazy<BumpAllocInner>,
}

struct BumpAllocInner {
    pages: PageFrame,
    marker: Mutex<PhysAddr>,
}

//

unsafe impl GlobalAlloc for BumpAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let inner = &*self.inner;

        let pages = &inner.pages;
        let mut marker = inner.marker.lock();

        let alloc_bottom = PhysAddr::new(align_up(marker.as_u64(), layout.align() as u64));
        let alloc_top = alloc_bottom + layout.size() as u64;

        if alloc_top > pages.addr() + pages.byte_len() as u64 {
            error!("OOM");
            error!("layout: {layout:?} pages: {pages:?} marker: {marker:?}");
            return null_mut();
        }

        *marker = alloc_top;

        to_higher_half(alloc_bottom).as_mut_ptr()
    }

    unsafe fn dealloc(&self, _ptr: *mut u8, _layout: Layout) {
        // BUMP alloc is stupid and won't free the memory
    }
}
