use core::{
    alloc::{GlobalAlloc, Layout},
    marker::PhantomData,
    ops::{Deref, DerefMut},
    ptr::NonNull,
    sync::atomic::{AtomicUsize, Ordering},
};

use lock_api::RawMutex;

use crate::{
    slab::{AllocMetadata, BigAllocMetadata},
    AllocBackend, PageFrames, Slab,
};

//

#[derive(Debug)]
pub struct SlabAllocatorStats {
    /// memory given out by this slab allocator
    pub used: AtomicUsize,
    /// physical memory allocated by this slab allocator
    pub allocated: AtomicUsize,
}

impl SlabAllocatorStats {
    pub const fn new() -> Self {
        Self {
            used: AtomicUsize::new(0),
            allocated: AtomicUsize::new(0),
        }
    }
}

//

pub struct SlabAllocator<P, Lock> {
    slabs: [Slab<P, Lock>; 13],
    stats: SlabAllocatorStats,

    _p: PhantomData<P>,
}

//

unsafe impl<P, Lock> GlobalAlloc for SlabAllocator<P, Lock>
where
    P: AllocBackend,
    Lock: RawMutex,
{
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        self.alloc(layout.size())
    }

    unsafe fn dealloc(&self, ptr: *mut u8, _layout: Layout) {
        if let Some(ptr) = NonNull::new(ptr) {
            unsafe { self.free(ptr) };
        }
    }
}

impl<P, Lock> SlabAllocator<P, Lock>
where
    Lock: RawMutex,
{
    #[cfg(not(all(loom, not(target_os = "none"))))]
    #[must_use]
    pub const fn new() -> Self {
        let slabs = [
            Slab::new(8),
            Slab::new(16),
            Slab::new(32),
            Slab::new(48),
            Slab::new(64),
            Slab::new(96),
            Slab::new(128),
            Slab::new(192),
            Slab::new(256),
            Slab::new(384),
            Slab::new(512),
            Slab::new(768),
            Slab::new(1024),
        ];

        // let mut idx = 0u8;
        // while idx < slabs.len() as u8 {
        //     slabs[idx as usize].idx = idx;
        // }

        Self {
            slabs,

            stats: SlabAllocatorStats {
                used: AtomicUsize::new(0),
                allocated: AtomicUsize::new(0),
            },

            _p: PhantomData,
        }
    }

    #[cfg(all(loom, not(target_os = "none")))]
    pub const fn new() -> Self {
        panic!("loom not supported");
    }

    pub fn get_slab(&self, size: usize) -> Option<(u8, &Slab<P, Lock>)> {
        self.slabs
            .iter()
            .enumerate()
            .find(|(_, slab)| slab.size >= size)
            .map(|(idx, slab)| (idx as u8, slab))
    }
}

impl<P, Lock> SlabAllocator<P, Lock>
where
    P: AllocBackend,
    Lock: RawMutex,
{
    pub fn alloc(&self, size: usize) -> *mut u8 {
        // crate::println!("alloc {size}");
        if let Some((idx, slab)) = self.get_slab(size) {
            slab.alloc(idx, &self.stats)
        } else {
            self.big_alloc(size)
        }
    }

    // TODO: Rust tells the layout on free, so I should optimize for that,
    // now the layout is figured out by reading the first block in a page
    /// # Safety
    /// `alloc` must point to an allocation that was previously allocated
    /// with this specific [`SlabAllocator`]
    pub unsafe fn free(&self, alloc: NonNull<u8>) {
        if alloc.as_ptr().is_aligned_to(0x1000) {
            return unsafe { self.big_free(alloc) };
        }

        let slab = self.slab_of(alloc);
        unsafe { slab.free(&self.stats, alloc) }
    }

    /// # Safety
    /// `alloc` must point to an allocation that was previously allocated
    /// with this specific [`SlabAllocator`]
    pub unsafe fn size(&self, alloc: NonNull<u8>) -> usize {
        if alloc.as_ptr().is_aligned_to(0x1000) {
            return unsafe { self.big_pages(alloc).1 * 0x1000 };
        }

        self.slab_of(alloc).size
    }

    fn slab_of(&self, alloc: NonNull<u8>) -> &Slab<P, Lock> {
        // align down to 0x1000
        // the first bytes in the page tells the slab size
        let page_alloc = ((alloc.as_ptr() as u64) & 0xFFFF_FFFF_FFFF_F000) as *mut u8;

        let header: AllocMetadata = unsafe { *(page_alloc as *const AllocMetadata) };

        header
            .idx()
            .and_then(|idx| self.slabs.get(idx as usize))
            .expect("alloc header to be valid")
    }

    fn big_alloc(&self, size: usize) -> *mut u8 {
        // minimum number of pages for the alloc + 1 page
        // for metadata
        let page_count = size.div_ceil(0x1000) + 1;
        let mut pages = P::alloc(page_count);

        self.stats.allocated.fetch_add(page_count, Ordering::SeqCst);
        self.stats
            .used
            .fetch_add(pages.byte_len(), Ordering::SeqCst);

        // write the big alloc metadata

        let metadata: &mut [BigAllocMetadata] =
            bytemuck::try_cast_slice_mut(pages.as_bytes_mut()).expect("allocation to be aligned");
        metadata[0] = BigAllocMetadata::new(page_count);

        // trace!("BigAlloc    {:#x} {size}", pages.addr().as_u64());

        // pmm already zeroed the memory
        //
        // the returned memory is the next page, because this page contains the metadata
        unsafe { pages.as_ptr().add(0x1000) }
    }

    /// # Safety
    /// The `v_addr` pointer must point to a big allocation that was previously allocated
    /// with this specific [`SlabAllocator`]
    unsafe fn big_free(&self, alloc: NonNull<u8>) {
        let (alloc, pages) = unsafe { self.big_pages(alloc) };
        let pages = unsafe { PageFrames::new(alloc, pages) };

        self.stats
            .allocated
            .fetch_sub(pages.len(), Ordering::SeqCst);
        self.stats
            .used
            .fetch_sub(pages.byte_len(), Ordering::SeqCst);

        // trace!("BigFree     {:#x} {size}", pages.addr().as_u64());

        P::free(pages);
    }

    unsafe fn big_pages(&self, alloc: NonNull<u8>) -> (*mut u8, usize) {
        let alloc = unsafe { alloc.as_ptr().sub(0x1000) };

        let metadata: BigAllocMetadata = unsafe { *(alloc as *const BigAllocMetadata) };
        let pages = metadata.size().expect("big alloc metadata to be valid");

        (alloc, pages)
    }
}

impl<P, Lock> Default for SlabAllocator<P, Lock>
where
    Lock: RawMutex,
{
    fn default() -> Self {
        Self::new()
    }
}

impl<P, Lock> Deref for SlabAllocator<P, Lock> {
    type Target = SlabAllocatorStats;

    fn deref(&self) -> &Self::Target {
        &self.stats
    }
}

impl<P, Lock> DerefMut for SlabAllocator<P, Lock> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.stats
    }
}
