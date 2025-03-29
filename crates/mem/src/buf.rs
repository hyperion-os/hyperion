use core::{
    fmt,
    marker::PhantomData,
    mem::MaybeUninit,
    ops::{Deref, Range},
    ptr,
};

use x86_64::VirtAddr;

use crate::vmm::PageMapImpl;

//

/* pub struct Buffer<T> {
    pages: Box<[PhysAddr]>,
    offset: usize,
    len: usize,
    _p: PhantomData<T>,
    // data: *const [T],
}

impl<T> Buffer<T> {
    /// # Safety
    /// pages must be owned
    pub unsafe fn new(page_map: &impl PageMapImpl, data: *const [T]) -> Option<Self> {
        let beg = VirtAddr::from_ptr(data.as_ptr());
        let end = beg + (data.len() * mem::size_of::<T>()) as u64;

        let PtrRangeInfo {
            aligned_beg,
            n_pages,
            ..
        } = ptr_range_info(beg..end);

        let pages = page_map.share_pages(aligned_beg, n_pages, PageTableFlags::USER_ACCESSIBLE)?;

        let offset = (beg - aligned_beg) as usize;
        let len = data.len();

        Some(Self {
            pages,
            offset,
            len,
            _p: PhantomData,
        })
    }

    pub fn iter(&self) -> impl Iterator<Item = *const T> {
        let () = self.pages.split_first();
    }

    pub fn read(&self, to: &mut [MaybeUninit<T>]) {
        debug_assert!(0x1000usize.rem_euclid(mem::align_of::<T>()) == 0);
        debug_assert!(0x1000usize.rem_euclid(mem::size_of::<T>()) == 0);

        ptr::read_volatile(src);
        active_map.activate();
    }
} */

/* impl<T> Buffer<T> {
    /// # Safety
    /// pages must be owned
    pub unsafe fn new(pages: Box<[PhysAddr]>, data: *const [T]) -> Self {
        Self { pages, data }
    }

    pub fn map<P: PageMapImpl>(&self, active_map: &P) -> BufferGuard<'_, T, P> {
        let tmp = active_map.alloc_temporary();
        tmp;

        BufferGuard { buf: self, tmp }
    }
}

#[must_not_suspend = "the guard is specific to one address space, and a suspend point might switch address spaces"]
pub struct BufferGuard<'a, T, P: PageMapImpl> {
    buf: &'a Buffer<T>,
    tmp: Temporary<'a, P>,
}

impl<T, P: PageMapImpl> Deref for BufferGuard<'_, T, P> {
    type Target = *const [T];

    fn deref(&self) -> &Self::Target {
        // TODO: debug assert is_active self.page_map.activate();
        &self.buf.data
    }
} */

pub struct Buffer<'a, T, P> {
    map: &'a P,
    ptr: usize,
    len: usize,
    _p: PhantomData<T>,
}

impl<'a, T, P: PageMapImpl> Buffer<'a, T, P> {
    /// # Safety
    /// the bytes have to be in the lower half,
    /// any page faults from there are safe and just kill the user process
    pub unsafe fn new(map: &'a P, ptr: usize, len: usize) -> Self {
        Self {
            map,
            ptr,
            len,
            _p: PhantomData,
        }
    }

    pub const fn len(&self) -> usize {
        self.len
    }

    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// # Safety
    /// calls to `Self::with_slice` or `PageMapImpl::activate` are not allowed inside `f`
    pub unsafe fn with_slice<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&[MaybeUninit<T>]) -> R,
    {
        // not async to make sure the thing stays mapped
        self.map.activate();

        let slice =
            unsafe { &*ptr::slice_from_raw_parts::<MaybeUninit<T>>(self.ptr as _, self.len) };
        f(slice)
    }
}

//

pub struct BufferMut<'a, T, P> {
    inner: Buffer<'a, T, P>,
}

impl<'a, T, P: PageMapImpl> BufferMut<'a, T, P> {
    /// # Safety
    /// the bytes have to be in the lower half,
    /// any page faults from there are safe and just kill the user process
    pub unsafe fn new(map: &'a P, ptr: usize, len: usize) -> Self {
        Self {
            inner: unsafe { Buffer::new(map, ptr, len) },
        }
    }

    /// # Safety
    /// calls to `Self::with_slice` or `PageMapImpl::activate` are not allowed inside `f`
    pub unsafe fn with_slice_mut<F, R>(&mut self, f: F) -> R
    where
        F: FnOnce(&mut [MaybeUninit<T>]) -> R,
    {
        // not async to make sure the thing stays mapped
        self.inner.map.activate();

        let slice = unsafe {
            &mut *ptr::slice_from_raw_parts_mut::<MaybeUninit<T>>(
                self.inner.ptr as _,
                self.inner.len,
            )
        };
        f(slice)
    }
}

impl<'a, P: PageMapImpl> BufferMut<'a, u8, P> {
    pub fn offset_writer(&'a mut self, from_offset: usize) -> BufferOffsetWriter<'a, P> {
        BufferOffsetWriter {
            from_offset,
            written: 0,
            buf: self,
        }
    }
}

impl<'a, T, P> Deref for BufferMut<'a, T, P> {
    type Target = Buffer<'a, T, P>;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

//

pub struct BufferOffsetWriter<'a, P> {
    from_offset: usize,
    pub written: usize,
    buf: &'a mut BufferMut<'a, u8, P>,
}

impl<P: PageMapImpl> BufferOffsetWriter<'_, P> {
    pub fn write(&mut self, bytes: &[u8]) {
        let from = self.from_offset;
        self.from_offset = self.from_offset.saturating_add(bytes.len());

        if from >= bytes.len() {
            return;
        }
        if self.written >= self.buf.len() {
            return;
        }

        unsafe {
            self.buf.with_slice_mut(|s| {
                let from = &bytes[from..];
                let to = &mut s[self.written..];

                let write = from.len().min(to.len());

                let from = &from[..write];
                let to = &mut to[..write];

                MaybeUninit::copy_from_slice(to, from);

                self.written = self.written.saturating_add(write);
            });
        }
    }
}

impl<P: PageMapImpl> fmt::Write for BufferOffsetWriter<'_, P> {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        self.write(s.as_bytes());
        Ok(())
    }
}

//

pub struct PtrRangeInfo {
    pub beg: VirtAddr,
    pub end: VirtAddr,
    pub inside_aligned_beg: VirtAddr,
    pub inside_aligned_end: VirtAddr,
    pub aligned_beg: VirtAddr,
    pub aligned_end: VirtAddr,
    pub first_page_size: u64,
    pub last_page_size: u64,
    pub n_full_pages: u64,
    pub n_pages: u64,
}

pub fn ptr_range_info(buf: Range<VirtAddr>) -> PtrRangeInfo {
    let beg = buf.start;
    let end = buf.end;

    let inside_aligned_beg = beg.align_up(0x1000u64);
    let inside_aligned_end = end.align_down(0x1000u64);

    let aligned_beg = beg.align_down(0x1000u64);
    let aligned_end = end.align_up(0x1000u64);

    let first_page_size = inside_aligned_end - beg;
    let last_page_size = end - inside_aligned_end;

    let n_full_pages = (inside_aligned_end - inside_aligned_beg) / 0x1000;
    let n_pages = (aligned_end - aligned_beg) / 0x1000;

    PtrRangeInfo {
        beg,
        end,
        inside_aligned_beg,
        inside_aligned_end,
        aligned_beg,
        aligned_end,
        first_page_size,
        last_page_size,
        n_full_pages,
        n_pages,
    }
}

/// splits the `buf` in page boundaries
pub fn page_split_iterator(buf: Range<VirtAddr>) -> impl Iterator<Item = Range<VirtAddr>> {
    let PtrRangeInfo {
        beg,
        end,
        inside_aligned_beg,
        inside_aligned_end,
        first_page_size,
        last_page_size,
        n_full_pages,
        ..
    } = ptr_range_info(buf);

    (first_page_size != 0)
        .then_some(beg..inside_aligned_beg)
        .into_iter()
        .chain((0..n_full_pages).map(move |i| {
            let page = inside_aligned_beg + i * 0x1000;
            page..page + 0x1000
        }))
        .chain((last_page_size != 0).then_some(inside_aligned_end..end))
}
