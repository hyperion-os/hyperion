use crate::{boot, debug, error, util::NumberPostfix};
use core::{
    alloc::{GlobalAlloc, Layout},
    ptr::null_mut,
    sync::atomic::{AtomicU64, Ordering},
};
use spin::Mutex;

//

pub fn init() {
    let mut usable = 0;

    for Memmap { base, len } in boot::memmap() {
        usable += len;
        debug!("base: {base:#X} len: {len:#X} ({}B)", len.postfix_binary());

        ALLOC.memory.store(base, Ordering::SeqCst);
        *ALLOC.remaining.lock() = len;
    }

    debug!("Usable system memory: {}B", usable.postfix_binary());
}

//

pub struct Memmap {
    pub base: u64,
    pub len: u64,
}

//

#[global_allocator]
static ALLOC: BumpAlloc = BumpAlloc {
    memory: AtomicU64::new(0),
    remaining: Mutex::new(0),
};

struct BumpAlloc {
    memory: AtomicU64,
    remaining: Mutex<u64>,
}

unsafe impl GlobalAlloc for BumpAlloc {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let memory = self.memory.load(Ordering::SeqCst);
        let mut remaining = self.remaining.lock();

        let top = memory + *remaining;
        let Some(tmp) = top.checked_sub(layout.size() as u64) else {
            error!("OUT OF MEMORY");
            error!(
                "ALLOC: size: {} align: {} top: {top} memory: {memory} remaining: {remaining}",
                layout.size(),
                layout.align()
            );
            return null_mut();
        };
        let new_top = tmp / layout.align() as u64 * layout.align() as u64;
        let reservation = top - new_top;

        if let Some(left) = remaining.checked_sub(reservation) {
            *remaining = left;
            (memory + left) as _
        } else {
            error!("OUT OF MEMORY");
            error!(
            "ALLOC: size: {} align: {} top: {top} new: {new_top} memory: {memory} remaining: {remaining}",
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

#[cfg(test)]
mod tests {
    use alloc::vec::Vec;

    #[test_case]
    fn test_alloc() {
        core::hint::black_box((0..64).map(|i| i * 2).collect::<Vec<_>>());
    }
}
