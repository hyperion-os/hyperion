//! Physical memory management
//!
//! Page frame allocating

use super::{map::Memmap, to_higher_half};
use crate::{
    boot, debug, trace,
    util::{bitmap::Bitmap, fmt::NumberPostfix},
};
use core::{
    fmt, slice,
    sync::atomic::{AtomicU64, AtomicUsize, Ordering},
};
use spin::{Lazy, Mutex};
use x86_64::{align_up, PhysAddr};

//

const PAGE_SIZE: u64 = 2u64.pow(12); // 4KiB pages

//

pub struct PageFrameAllocator {
    // 1 bits are used pages
    bitmap: Mutex<Bitmap<'static>>,
    usable: AtomicU64,
    used: AtomicU64,
    total: AtomicU64,

    last_alloc_index: AtomicUsize,
}

#[derive(Debug)]
pub struct PageFrame {
    first: PhysAddr,
    count: usize,
}

//

impl PageFrameAllocator {
    pub fn get() -> &'static PageFrameAllocator {
        static PFA: Lazy<PageFrameAllocator> = Lazy::new(PageFrameAllocator::init);
        &PFA
    }

    /// System total memory in bytes
    pub fn total_mem(&self) -> u64 {
        self.total.load(Ordering::SeqCst)
    }

    /// System usable memory in bytes
    pub fn usable_mem(&self) -> u64 {
        self.usable.load(Ordering::SeqCst)
    }

    /// Currently used usable memory in bytes
    pub fn used_mem(&self) -> u64 {
        self.used.load(Ordering::SeqCst)
    }

    /// Currently free usable memory in bytes
    pub fn free_mem(&self) -> u64 {
        self.usable_mem() - self.used_mem()
    }

    /// Reserved memory in bytes
    pub fn reserved_mem(&self) -> u64 {
        self.total_mem() - self.usable_mem()
    }

    /// Free up pages
    ///
    /// Double frees are not possible due to [`PageFrame`] missing [`Clone`] and it cannot be
    /// constructed manually
    pub fn free(&self, frame: PageFrame) {
        if frame.first.as_u64() == 0 || frame.count == 0 {
            return;
        }

        let mut bitmap = self.bitmap.lock();
        let page = (frame.first.as_u64() / PAGE_SIZE) as usize;
        for page in page..page + frame.count {
            bitmap.set(page, false).unwrap();
        }

        self.used
            .fetch_sub(frame.count as u64 * PAGE_SIZE, Ordering::SeqCst);
    }

    /// Alloc pages
    ///
    /// Use [`Self::free`] to not leak pages (-> memory)
    pub fn alloc(&self, count: usize) -> PageFrame {
        if count == 0 {
            return PageFrame {
                first: PhysAddr::new(0),
                count: 0,
            };
        }

        let mut bitmap = self.bitmap.lock();

        let first_page = self.alloc_at(&mut bitmap, count).unwrap_or_else(|| {
            // TODO: handle OOM a bit better
            self.alloc_from(0);
            self.alloc_at(&mut bitmap, count).expect("OOM")
        });

        self.alloc_from(first_page + count);

        let addr = PhysAddr::new(first_page as u64 * PAGE_SIZE);

        // SAFETY: TODO:
        let page_data: &mut [u8] = unsafe {
            slice::from_raw_parts_mut(
                to_higher_half(addr).as_mut_ptr(),
                count * PAGE_SIZE as usize,
            )
        };

        // fill the page with zeros
        trace!("Memzeroing {:?}", page_data.as_ptr_range());
        page_data.fill(0);

        self.used
            .fetch_add(count as u64 * PAGE_SIZE, Ordering::SeqCst);

        PageFrame { first: addr, count }
    }

    fn alloc_from(&self, index: usize) {
        self.last_alloc_index.store(index, Ordering::SeqCst)
    }

    // returns the page index, not the page address
    fn alloc_at(&self, bitmap: &mut Bitmap, count: usize) -> Option<usize> {
        let mut first_page = self.last_alloc_index.load(Ordering::SeqCst);
        'main: loop {
            if first_page + count > bitmap.len() {
                return None;
            }

            /* if test_log_level(LogLevel::Trace) {
                trace!(
                    "Trying to allocate {count} pages from {:?}",
                    to_higher_half(PhysAddr::new(first_page as u64 * PAGE_SIZE))
                );
            } */

            // go reversed so that skips would be more efficient
            for offs in (0..count).rev() {
                // SAFETY: `first_page + offs` < `first_page + count` <= `bitmap.len()`
                // => bitmap has to contain `first_page + offs`
                if unsafe { bitmap.get(first_page + offs).unwrap_unchecked() } {
                    // skip all page windows which have this locked page
                    first_page = first_page + offs + 1;
                    continue 'main;
                }
            }

            // found a window of free pages
            for offs in 0..count {
                // lock them
                _ = bitmap.set(first_page + offs, true);
            }

            return Some(first_page);
        }
    }

    fn init() -> Self {
        // usable system memory
        let usable: u64 = boot::memmap()
            .filter(Memmap::is_usable)
            .map(|Memmap { len, .. }| len)
            .sum();

        // total system memory
        let total: u64 = boot::memmap().map(|Memmap { len, .. }| len).sum();

        // the end of the usable physical memory address space
        let top = boot::memmap()
            .filter(Memmap::is_usable)
            .map(|Memmap { base, len, ty: _ }| base + len)
            .max()
            .expect("No memory");

        // size in bytes
        let bitmap_size = align_up(top.as_u64() / PAGE_SIZE / 8, PAGE_SIZE);
        let bitmap_data = boot::memmap()
            .filter(Memmap::is_usable)
            .find(|Memmap { len, .. }| *len >= bitmap_size)
            .expect("No place to store PageFrameAllocator bitmap")
            .base;

        // SAFETY: this bitmap is going to be initialized before it is read from
        let bitmap = unsafe {
            slice::from_raw_parts_mut(to_higher_half(bitmap_data).as_mut_ptr(), bitmap_size as _)
        };
        let mut bitmap = Bitmap::new(bitmap);
        bitmap.fill(true); // initialized here

        // free up some pages
        for Memmap {
            mut base,
            mut len,
            ty: _,
        } in boot::memmap().filter(Memmap::is_usable)
        {
            if base == bitmap_data {
                // skip the bitmap allocation spot
                base += bitmap_data.as_u64();
                len -= bitmap_size;
            }

            let mut bottom = base.as_u64();
            let mut top = base.as_u64() + len;

            debug!(
                "Free pages: {:#0X?} ({}B)",
                bottom..top,
                (top - bottom).postfix_binary()
            );

            bottom /= PAGE_SIZE;
            top /= PAGE_SIZE;

            for page in bottom..top {
                bitmap.set(page as _, false).unwrap();
            }
        }

        let pfa = Self {
            bitmap: bitmap.into(),
            usable: usable.into(),
            used: bitmap_size.into(),
            total: total.into(),

            last_alloc_index: 0.into(),
        };

        debug!("PFA initialized:\n{pfa}");

        pfa
    }
}

impl fmt::Display for PageFrameAllocator {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(
            f,
            "Total system memory: {}B",
            self.total_mem().postfix_binary()
        )?;
        writeln!(
            f,
            "Usable system memory: {}B",
            self.usable_mem().postfix_binary()
        )?;
        writeln!(
            f,
            "Used system memory: {}B",
            self.used_mem().postfix_binary()
        )?;
        writeln!(
            f,
            "Free system memory: {}B",
            self.free_mem().postfix_binary()
        )?;
        write!(
            f,
            "Reserved system memory: {}B",
            self.reserved_mem().postfix_binary()
        )?;

        Ok(())
    }
}

impl PageFrame {
    // physical address of the first page
    pub fn addr(&self) -> PhysAddr {
        self.first
    }

    /// number of pages
    pub fn len(&self) -> usize {
        self.count
    }

    /// number of bytes
    pub fn byte_len(&self) -> usize {
        self.count * PAGE_SIZE as usize
    }

    pub fn as_bytes_mut(&mut self) -> &mut [u8] {
        // SAFETY: &mut self makes sure that this is the only safe mut ref
        unsafe { self.as_bytes_mut_unsafe() }
    }

    pub fn as_bytes(&self) -> &[u8] {
        // SAFETY: the mut ref is immediately downgraded to a const ref
        unsafe { self.as_bytes_mut_unsafe() }
    }

    /// SAFETY: only 1 mutable slice at one time
    unsafe fn as_bytes_mut_unsafe(&self) -> &mut [u8] {
        slice::from_raw_parts_mut(to_higher_half(self.first).as_mut_ptr(), self.byte_len())
    }
}

//

#[cfg(test)]
mod tests {
    use super::PageFrameAllocator;

    #[test_case]
    fn pfa_simple() {
        let pfa = PageFrameAllocator::get();

        let a = pfa.alloc(1);
        assert_ne!(a.addr().as_u64(), 0);

        let b = pfa.alloc(1);
        assert_ne!(b.addr().as_u64(), 0);
        assert_ne!(a.addr().as_u64(), b.addr().as_u64());

        pfa.free(a);
        pfa.alloc_from(0);
        let c = pfa.alloc(1);
        assert_ne!(c.addr().as_u64(), 0);
        assert_ne!(b.addr().as_u64(), c.addr().as_u64());

        let d = pfa.alloc(1);
        assert_ne!(d.addr().as_u64(), 0);
        assert_ne!(c.addr().as_u64(), d.addr().as_u64());

        // pfa.free(a); // <- compile error as expected
        pfa.free(b);
        pfa.free(c);
        pfa.free(d);
    }
}
