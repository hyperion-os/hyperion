use super::{map::Memmap, to_higher_half};
use crate::{boot, error};
use core::{
    alloc::{GlobalAlloc, Layout},
    ptr::null_mut,
};
use spin::{Mutex, Once};

//

const MAX_BUMP_ALLOC: u64 = 2u64.pow(16); // 64KiB

//

pub fn init() {
    let mut map = boot::memmap()
        .min_by_key(|Memmap { len, .. }| *len)
        .expect("No memory");

    map.len = map.len.min(MAX_BUMP_ALLOC);

    ALLOC.inner.call_once(|| BumpAllocInner {
        remaining: Mutex::new(map.len),
        map,
    });
}

pub fn map() -> Option<Memmap> {
    ALLOC.inner.get().map(|i| i.map)
}

//

#[global_allocator]
static ALLOC: BumpAllocator = BumpAllocator { inner: Once::new() };

//

struct BumpAllocator {
    inner: Once<BumpAllocInner>,
}

struct BumpAllocInner {
    map: Memmap,
    remaining: Mutex<u64>,
}

//

unsafe impl GlobalAlloc for BumpAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let Some(inner) = self.inner.get() else {
            error!("Allocator used before init");
            return null_mut();
        };

        let memory = inner.map.base;
        let mut remaining = inner.remaining.lock();

        let top = (memory + *remaining).as_u64();
        let Some(tmp) = top.checked_sub(layout.size() as u64) else {
            error!("OUT OF MEMORY");
            error!(
                "ALLOC: size: {} align: {} top: {top} memory: {memory:?} remaining: {remaining}",
                layout.size(),
                layout.align()
                );
            return null_mut();
        };
        let new_top = tmp / layout.align() as u64 * layout.align() as u64;
        let reservation = top - new_top;

        if let Some(left) = remaining.checked_sub(reservation) {
            *remaining = left;
            to_higher_half(memory + left).as_mut_ptr()
        } else {
            error!("OUT OF MEMORY");
            error!(
                "ALLOC: size: {} align: {} top: {top} new: {new_top} memory: {memory:?} remaining: {remaining}",
                layout.size(),
                layout.align()
            );
            null_mut()
        }
    }

    unsafe fn dealloc(&self, _ptr: *mut u8, _layout: Layout) {
        // BUMP alloc is stupid and won't free the memory
    }
}
