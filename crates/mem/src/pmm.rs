//! Physical memory management
//!
//! Page frame allocating

use alloc::vec::Vec;
use core::{
    alloc::{AllocError, Allocator, Layout},
    fmt,
    mem::{transmute, MaybeUninit},
    ptr::NonNull,
    slice,
    sync::atomic::{AtomicUsize, Ordering},
};

use hyperion_bitmap::{AtomicBitmap, Bitmap};
use hyperion_boot::memmap;
use hyperion_boot_interface::Memmap;
use hyperion_log::debug;
use hyperion_num_postfix::NumberPostfix;
use spin::Lazy;
use x86_64::{align_up, structures::paging::PhysFrame, PhysAddr, VirtAddr};

use super::{from_higher_half, to_higher_half};

//

pub static PFA: Lazy<PageFrameAllocator> = Lazy::new(PageFrameAllocator::init);

const PAGE_SIZE: usize = 2usize.pow(12); // 4KiB pages

//

pub struct PageFrameAllocator {
    // 1 bits are used pages
    bitmap: AtomicBitmap<'static>,
    usable: AtomicUsize,
    used: AtomicUsize,
    total: AtomicUsize,

    last_alloc_index: AtomicUsize,
}

#[derive(Debug)]
pub struct PageFrame {
    first: PhysAddr,
    count: usize,
}

//

impl PageFrameAllocator {
    /// System total memory in bytes
    pub fn total_mem(&self) -> usize {
        self.total.load(Ordering::SeqCst)
    }

    /// System usable memory in bytes
    pub fn usable_mem(&self) -> usize {
        self.usable.load(Ordering::SeqCst)
    }

    /// Currently used usable memory in bytes
    pub fn used_mem(&self) -> usize {
        self.used.load(Ordering::SeqCst)
    }

    /// Currently free usable memory in bytes
    pub fn free_mem(&self) -> usize {
        self.usable_mem() - self.used_mem()
    }

    /// Reserved memory in bytes
    pub fn reserved_mem(&self) -> usize {
        self.total_mem() - self.usable_mem()
    }

    pub fn bitmap_len(&self) -> usize {
        self.bitmap.len()
    }

    /// Free up pages
    pub fn free(&self, mut frame: PageFrame) {
        if frame.first.as_u64() == 0 || frame.count == 0 {
            return;
        }

        let page = frame.first.as_u64() as usize / PAGE_SIZE;
        for page in page..page + frame.count {
            assert_eq!(
                self.bitmap.swap(page, false, Ordering::Release),
                Some(true),
                "trying to free pages that were already free"
            );
        }
        // trace!("freeing pages first={page} count={}", frame.count);

        frame.as_bytes_mut().fill(0);

        self.used
            .fetch_sub(frame.count * PAGE_SIZE, Ordering::SeqCst);
    }

    /// Alloc pages
    ///
    /// Use [`Self::free`] to not leak pages (-> memory)
    // #[track_caller]
    pub fn alloc(&self, count: usize) -> PageFrame {
        if count == 0 {
            return PageFrame {
                first: PhysAddr::new(0),
                count: 0,
            };
        };

        /* hyperion_log::debug!(
            "allocating {count} pages (from {})",
            core::panic::Location::caller()
        ); */

        // TODO: lock-less page alloc
        let from = self.last_alloc_index.load(Ordering::SeqCst);
        let first_page = self.alloc_at(from, count).unwrap_or_else(|| {
            // TODO: handle OOM a bit better
            self.last_alloc_index.store(0, Ordering::SeqCst);
            self.alloc_at(0, count).expect("OOM")
        });

        self.last_alloc_index
            .store(first_page + count, Ordering::SeqCst);

        let addr = PhysAddr::new((first_page * PAGE_SIZE) as u64);
        let page_ptr: *mut MaybeUninit<u8> = to_higher_half(addr).as_mut_ptr();
        assert!(
            page_ptr.is_aligned_to(PAGE_SIZE),
            "pages should be aligned to {PAGE_SIZE}"
        );

        // Safety: the pages get protected from allocations
        let page_data: &mut [MaybeUninit<u8>] =
            unsafe { slice::from_raw_parts_mut(page_ptr, count * PAGE_SIZE) };
        /* let page_data = */
        fill_maybeuninit_slice(page_data, 0);

        self.used.fetch_add(count * PAGE_SIZE, Ordering::SeqCst);

        PageFrame { first: addr, count }
    }

    // returns the page index, not the page address
    fn alloc_at(&self, from: usize, count: usize) -> Option<usize> {
        // let mut first_page = self.last_alloc_index.load(Ordering::SeqCst);

        let mut first_page = from;
        let bitmap_len = self.bitmap.len();

        'main: loop {
            if first_page + count > bitmap_len {
                return None;
            }

            // TODO: lock the pages in reverse as a small optimization
            for offs in 0..count {
                let page = first_page + offs;

                // lock the window
                if self.bitmap.swap(page, true, Ordering::Acquire).unwrap() {
                    // 0..offs, means that the last page that we couldn't acquire, won't be freed
                    for offs in 0..offs {
                        let page = first_page + offs;
                        // the first swap already acquired exclusive access to these pages
                        // so they are safe to free
                        // TODO: unsafe fn
                        self.bitmap.swap(page, false, Ordering::Release).unwrap();
                    }

                    first_page = first_page + offs + 1;
                    continue 'main;
                }
            }

            return Some(first_page);
        }
    }

    fn init() -> Self {
        // usable system memory
        let usable: usize = memmap()
            .filter(Memmap::is_usable)
            .map(|Memmap { len, .. }| len)
            .sum();

        // total system memory
        let total: usize = memmap().map(|Memmap { len, .. }| len).sum();

        // the end of the usable physical memory address space
        let top = memmap()
            .filter(Memmap::is_usable)
            .map(|Memmap { base, len, ty: _ }| base + len)
            .max()
            .expect("No memory");

        // size in bytes
        let bitmap_size: usize = align_up((top / PAGE_SIZE / 8) as _, PAGE_SIZE as _) as _;
        let bitmap_data: usize = memmap()
            .filter(Memmap::is_usable)
            .find(|Memmap { len, .. }| *len >= bitmap_size)
            .expect("No place to store PageFrameAllocator bitmap")
            .base;
        let bitmap_ptr: *mut MaybeUninit<u8> =
            to_higher_half(PhysAddr::new(bitmap_data as _)).as_mut_ptr();

        // SAFETY: this bitmap is going to be initialized before it is read from
        // the memory region also gets protected from allocations
        let bitmap: &mut [MaybeUninit<u8>] =
            unsafe { slice::from_raw_parts_mut(bitmap_ptr, bitmap_size as _) };
        let bitmap = fill_maybeuninit_slice(bitmap, 0xFF);
        let bitmap = AtomicBitmap::from_mut(bitmap);
        bitmap.fill(true, Ordering::SeqCst);
        // bitmap.fill(true, Ordering::SeqCst); // initialized here

        // free up some pages
        for Memmap {
            mut base,
            mut len,
            ty: _,
        } in memmap().filter(Memmap::is_usable)
        {
            if base == bitmap_data {
                // skip the bitmap allocation spot
                base += bitmap_size;
                len -= bitmap_size;
            }

            let mut bottom = base;
            let mut top = base + len;

            debug!(
                "Free pages: [ {:#018x?} ] ({}B)",
                bottom..top,
                (top - bottom).postfix_binary()
            );

            bottom /= PAGE_SIZE;
            top /= PAGE_SIZE;

            for page in bottom..top {
                bitmap.store(page, false, Ordering::Release).unwrap();
            }
        }

        let pfa = Self {
            bitmap,
            usable: usable.into(),
            used: bitmap_size.into(),
            total: total.into(),

            last_alloc_index: 0.into(),
        };

        debug!("PFA initialized:\n{pfa}");

        pfa
    }
}

unsafe impl Allocator for PageFrameAllocator {
    fn allocate(&self, layout: Layout) -> Result<NonNull<[u8]>, AllocError> {
        let frame = self.alloc(layout.size() / PAGE_SIZE);

        NonNull::new(frame.virtual_addr().as_mut_ptr())
            .map(|first| NonNull::slice_from_raw_parts(first, frame.byte_len()))
            .ok_or(AllocError)
    }

    unsafe fn deallocate(&self, ptr: NonNull<u8>, layout: Layout) {
        self.free(PageFrame {
            first: from_higher_half(VirtAddr::new(ptr.as_ptr() as u64)),
            count: layout.size() / PAGE_SIZE,
        })
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
    /// # Safety
    ///
    /// The caller has to make sure that it has exclusive access to bytes in physical memory range
    /// `first..first + PAGE_SIZE * count`
    pub const unsafe fn new(first: PhysAddr, count: usize) -> Self {
        Self { first, count }
    }

    /// physical address of the first page
    pub const fn physical_addr(&self) -> PhysAddr {
        self.first
    }

    pub fn virtual_addr(&self) -> VirtAddr {
        to_higher_half(self.first)
    }

    /// number of pages
    pub const fn len(&self) -> usize {
        self.count
    }

    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// number of bytes
    pub const fn byte_len(&self) -> usize {
        self.count * PAGE_SIZE
    }

    pub fn as_bytes(&self) -> &[u8] {
        let addr = self.virtual_addr().as_mut_ptr();

        // Safety:
        // &mut self makes sure that this is the only safe mut ref
        // The page frame allocator gave exclusive access to these bytes
        unsafe { slice::from_raw_parts(addr, self.byte_len()) }
    }

    pub fn as_bytes_mut(&mut self) -> &mut [u8] {
        let addr = self.virtual_addr().as_mut_ptr();

        // Safety:
        // The page frame allocator gave exclusive access to these bytes
        unsafe { slice::from_raw_parts_mut(addr, self.byte_len()) }
    }

    /// Leak the PageFrame to get a static mut ref to it
    ///
    /// # Note
    ///
    /// page frames are not deallocated automatically anyways,
    /// this just takes ownership to give a safe method of getting a static ref to the data
    pub fn leak(mut self) -> &'static mut [u8] {
        unsafe { transmute(self.as_bytes_mut()) }
    }
}

//

fn fill_maybeuninit_slice<T: Copy>(s: &mut [MaybeUninit<T>], v: T) -> &mut [T] {
    s.fill(MaybeUninit::new(v));

    // Safety: The whole slice has been filled with copies of `v`
    unsafe { MaybeUninit::slice_assume_init_mut(s) }
}

#[cfg(test)]
mod tests {

    /* #[test]
    fn pfa_simple() {
        let a = PFA.alloc(1);
        assert_ne!(a.physical_addr().as_u64(), 0);

        let b = PFA.alloc(1);
        assert_ne!(b.physical_addr().as_u64(), 0);
        assert_ne!(a.physical_addr().as_u64(), b.physical_addr().as_u64());

        PFA.free(a);
        PFA.alloc_from(0);
        let c = PFA.alloc(1);
        assert_ne!(c.physical_addr().as_u64(), 0);
        assert_ne!(b.physical_addr().as_u64(), c.physical_addr().as_u64());

        let d = PFA.alloc(1);
        assert_ne!(d.physical_addr().as_u64(), 0);
        assert_ne!(c.physical_addr().as_u64(), d.physical_addr().as_u64());

        // PFA.free(a); // <- compile error as expected
        PFA.free(b);
        PFA.free(c);
        PFA.free(d);
    } */
}
