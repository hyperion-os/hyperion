use super::map::Memmap;
use crate::{
    boot, debug,
    log::{test_log_level, LogLevel},
    mem::bump,
    util::{bitmap::Bitmap, fmt::NumberPostfix},
};
use core::slice;
use spin::{Mutex, Once};
use x86_64::{align_down, align_up};

//

const PAGE_SIZE: u64 = 2u64.pow(12); // 4KiB pages

// const PAGE_SIZE: u64 = 2u64.pow(21); // 2MiB pages

static PFA: Once<Mutex<PageFrameAllocator>> = Once::new();

//

pub fn init() {
    let mem_bottom = boot::memmap()
        .map(|Memmap { base, .. }| base)
        .min()
        .expect("No memory");

    let mem_top = boot::memmap()
        .map(|Memmap { base, len }| base + len)
        .max()
        .expect("No memory");

    // size in bytes
    let bitmap_size = (mem_top - mem_bottom) / PAGE_SIZE / 8 + 1;
    let bitmap_data = boot::memmap()
        .find(|Memmap { len, .. }| *len >= bitmap_size)
        .expect("No place to store PageFrameAllocator bitmap")
        .base;

    // SAFETY: this bitmap is going to be initialized before it is read from
    let bitmap = unsafe { slice::from_raw_parts_mut(bitmap_data as _, bitmap_size as _) };
    let mut bitmap = Bitmap::new(bitmap);
    bitmap.fill(true); // initialized here

    let bottom_page = align_up(mem_bottom, PAGE_SIZE) / PAGE_SIZE;

    // free up some pages
    for Memmap { mut base, mut len } in boot::memmap() {
        if let Some(map) = bump::map() && map.base == base {
            // skip the BumpAllocator spot
            base += map.base;
            len -= map.len;
        }
        if base == bitmap_data {
            // skip the bitmap allocation spot
            base += bitmap_data;
            len -= bitmap_size;
        }

        let mut bottom = align_up(base, PAGE_SIZE);
        let mut top = align_down(base + len, PAGE_SIZE);

        if bottom >= top {
            continue;
        }

        debug!(
            "Free pages: {:#0X?} ({}B)",
            bottom..top,
            (top - bottom).postfix_binary()
        );

        bottom /= PAGE_SIZE;
        top /= PAGE_SIZE;
        bottom -= bottom_page;
        top -= bottom_page;

        for page in bottom..top {
            #[cfg(debug_assertions)]
            bitmap.set(page as _, false).unwrap();
            #[cfg(not(debug_assertions))]
            let _ = bitmap.set(page as _, false);
        }
    }

    let free = bitmap.iter_false().count() as u64 * PAGE_SIZE;
    let used = 0;
    debug!("Free pages: ({}B)", free.postfix_binary());

    PFA.call_once(|| {
        Mutex::new(PageFrameAllocator {
            bitmap,
            free,
            used,
            bottom_page,
        })
    });
}

//

pub struct PageFrameAllocator {
    bitmap: Bitmap<'static>,
    free: u64,
    used: u64,
    bottom_page: u64,
}

//

impl PageFrameAllocator {
    pub fn free_page(&mut self, addr: u64) {}
}
