use core::{
    alloc::GlobalAlloc,
    ptr::{self, NonNull},
    sync::atomic::{AtomicUsize, Ordering},
};

use hyperion_slab_alloc::{PageAlloc, Pages, SlabAllocator};
use hyperion_syscall::{fs::FileDesc, mem_map, mem_unmap, MemMapFlags};

//

// FIXME: allow mem_map to find a slot instead of replacing
static HEAP_BOTTOM: AtomicUsize = AtomicUsize::new(0x1000_0000_0000);

//

pub struct PageAllocator;

unsafe impl GlobalAlloc for PageAllocator {
    unsafe fn alloc(&self, layout: core::alloc::Layout) -> *mut u8 {
        let pages = layout.size().div_ceil(0x1000);
        unsafe { <PageAllocator as PageAlloc>::alloc(pages).as_ptr() }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: core::alloc::Layout) {
        let pages = layout.size().div_ceil(0x1000);
        unsafe { <PageAllocator as PageAlloc>::dealloc(Pages::new(ptr, pages)) }
    }
}

unsafe impl PageAlloc for PageAllocator {
    unsafe fn alloc(pages: usize) -> Pages {
        let alloc_addr = HEAP_BOTTOM.fetch_add(pages * 0x1000, Ordering::Relaxed);

        let first = mem_map(
            NonNull::new(alloc_addr as _),
            pages * 0x1000,
            MemMapFlags::HEAP,
            FileDesc(0),
            0,
        )
        .map_or(ptr::null_mut(), |ptr| ptr.cast().as_ptr());

        unsafe { Pages::new(first, pages) }
    }

    unsafe fn dealloc(frames: Pages) {
        if let Some(ptr) = NonNull::new(frames.as_ptr()) {
            _ = mem_unmap(ptr.cast(), frames.byte_len());
        }
    }
}

//

pub type SlabAlloc = SlabAllocator<PageAllocator>;

#[global_allocator]
pub static GLOBAL_ALLOC: SlabAlloc = SlabAlloc::new();
// static GLOBAL_ALLOC: PageAlloc = PageAlloc;
