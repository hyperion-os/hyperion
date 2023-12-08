//! Slab allocator
//!
//! - Allocates pages from [`PageFrameAllocator`].
//! - Splits pages into blocks with predefined sizes and builds a linked list using them.
//! - [`SlabAllocator::alloc`] gives the smallest of these blocks that is larger than the asked size.
//! The alignment is at least the same as the asked size. (up to 4KiB)
//! - Bigger allocations waste one page of memory to keep track of the allocation metadata.
//!
//! Note: the current implementation doesn't ever free the pages

#![no_std]
#![feature(pointer_is_aligned, const_pointer_is_aligned)]

//

use core::{
    alloc::{GlobalAlloc, Layout},
    marker::PhantomData,
    mem::size_of,
    ptr::{null_mut, NonNull},
    slice,
    sync::atomic::{AtomicUsize, Ordering},
};

use bytemuck::{Pod, Zeroable};
use lock_api::{Mutex, RawMutex};

//

const PAGE_SIZE: usize = 0x1000; // 4KiB pages

//

pub trait PageFrameAllocator {
    fn alloc(pages: usize) -> PageFrames;

    fn free(frames: PageFrames);
}

//

pub struct PageFrames {
    first: *mut u8,
    len: usize,
}

impl PageFrames {
    /// # Safety
    /// `first` must point to a valid page allocation of `len * 0x1000` bytes
    pub const unsafe fn new(first: *mut u8, len: usize) -> Self {
        debug_assert!(first.is_aligned_to(0x1000));
        Self { first, len }
    }

    pub const fn len(&self) -> usize {
        self.len
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub const fn byte_len(&self) -> usize {
        self.len() * PAGE_SIZE
    }

    pub const fn as_bytes(&self) -> &[u8] {
        unsafe { slice::from_raw_parts(self.first as *const u8, self.byte_len()) }
    }

    pub fn as_bytes_mut(&mut self) -> &mut [u8] {
        unsafe { slice::from_raw_parts_mut(self.first, self.byte_len()) }
    }

    pub const fn as_ptr(&self) -> *mut u8 {
        self.first
    }
}

//

pub struct SlabAllocator<P, Lock> {
    // TODO: lock-free
    slabs: [Slab<P, Lock>; 13],
    stats: SlabAllocatorStats,

    _p: PhantomData<P>,
}

#[derive(Debug)]
pub struct SlabAllocatorStats {
    /// memory given out by this slab allocator
    used: AtomicUsize,
    /// physical memory allocated by this slab allocator
    allocated: AtomicUsize,
}

pub struct Slab<P, Lock> {
    pub size: usize,

    // head: Mutex<Lock, Option<NonNull<Node>>>,
    next: Mutex<Lock, *mut u8>,

    _p: PhantomData<P>,
}

// DST, `size_of::<Self>() == Slab.size`
// struct Node {
//     next: Option<NonNull<Self>>,
//     data: [u8; 0],
// }

unsafe impl<P, Lock: Sync> Sync for Slab<P, Lock> {}
unsafe impl<P, Lock: Send> Send for Slab<P, Lock> {}

//

unsafe impl<P, Lock> GlobalAlloc for SlabAllocator<P, Lock>
where
    P: PageFrameAllocator,
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
    P: PageFrameAllocator,
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

        // align down to 0x1000
        // the first bytes in the page tells the slab size
        let page_alloc = ((alloc.as_ptr() as u64) & 0xFFFFFFFFFFFFF000) as *mut u8;

        let header: AllocMetadata = unsafe { *(page_alloc as *const AllocMetadata) };

        let slab = header
            .idx()
            .and_then(|idx| self.slabs.get(idx as usize))
            .expect("alloc header to be valid");

        unsafe { slab.free(&self.stats, alloc) };
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
        let alloc = unsafe { alloc.as_ptr().sub(0x1000) };

        let metadata: BigAllocMetadata = unsafe { *(alloc as *const BigAllocMetadata) };
        let pages = metadata.size().expect("big alloc metadata to be valid");

        let pages = unsafe { PageFrames::new(alloc, pages) };

        self.stats
            .allocated
            .fetch_sub(pages.len(), Ordering::SeqCst);
        self.stats
            .used
            .fetch_sub(pages.byte_len(), Ordering::SeqCst);

        // trace!("BigFree     {:#x} {size}", pages.addr().as_u64());

        P::free(pages)
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

impl<P, Lock> Slab<P, Lock>
where
    Lock: RawMutex,
{
    pub const fn new(size: usize) -> Self {
        assert!(
            size >= size_of::<u64>() && size % size_of::<u64>() == 0,
            "slab size should be a multiple of u64's size (8 bytes) and not zero"
        );

        Self {
            size,
            next: Mutex::new(null_mut()),
            _p: PhantomData,
        }
    }
}

impl<P, Lock> Slab<P, Lock>
where
    P: PageFrameAllocator,
    Lock: RawMutex,
{
    /// pop a block from the linked list
    pub fn pop(&self, idx: u8, stats: &SlabAllocatorStats) -> *mut u8 {
        let mut next = self.next.lock();

        // hyperion_log::trace!("allocating {}", self.size);
        let block = if !next.is_null() {
            // hyperion_log::trace!("using a preallocated slab");
            *next
        } else {
            Self::next_block(idx, self.size, stats)
        };

        let block_data: &mut [u64] =
            unsafe { slice::from_raw_parts_mut(block as *mut u64, self.size / 8) };
        *next = block_data[0] as _;
        block_data[0] = 0; // zero out the first u64 that was used as the 'next' pointer

        block
    }

    fn next_block(idx: u8, size: usize, stats: &SlabAllocatorStats) -> *mut u8 {
        let mut page = P::alloc(1);
        stats.allocated.fetch_add(1, Ordering::Relaxed);
        // let page_bytes = page.byte_len();
        // let page = to_higher_half(page.addr());

        let block_parts: &mut [u64] = bytemuck::cast_slice_mut(page.as_bytes_mut());
        let mut blocks = block_parts.chunks_exact_mut(size / size_of::<u64>());

        // write header

        let header = &mut blocks.next().expect("Slab size too large")[0];
        let header: &mut AllocMetadata = bytemuck::cast_mut(header);
        *header = AllocMetadata::new(idx);

        // create a linked list out of the slabs

        let mut first = None::<*mut u8>;
        let mut prev = None::<&mut [u64]>;
        for next in blocks {
            let addr = next.as_ptr() as _;
            if first.is_none() {
                first = Some(addr);
            }
            if let Some(prev) = prev {
                prev[0] = addr as u64;
            }

            prev = Some(next);
        }

        let (Some(first), Some(prev)) = (first, prev) else {
            panic!("Slab size too large");
        };

        prev[0] = 0;

        first
    }

    pub fn push(&self, block: NonNull<u8>) {
        let block_data: &mut [u64] =
            unsafe { slice::from_raw_parts_mut(block.as_ptr() as *mut u64, self.size / 8) };
        block_data.fill(0);

        let mut next = self.next.lock();
        block_data[0] = *next as u64;
        *next = block.as_ptr();
    }

    pub fn alloc(&self, idx: u8, stats: &SlabAllocatorStats) -> *mut u8 {
        stats.used.fetch_add(self.size, Ordering::Relaxed);
        self.pop(idx, stats)
    }

    /// # Safety
    /// `block` must point to an allocation that was previously allocated
    /// with this specific [`Slab`]
    pub unsafe fn free(&self, stats: &SlabAllocatorStats, block: NonNull<u8>) {
        stats.used.fetch_sub(self.size, Ordering::Relaxed);
        self.push(block)
    }
}

//

#[derive(Debug, Clone, Copy, Pod, Zeroable)]
#[repr(C)]
struct BigAllocMetadata {
    // a magic number to make it more likely to expose bugs
    magic: u64,

    // size of the alloc in bytes
    size: usize,
}

const _: () = assert!(core::mem::size_of::<BigAllocMetadata>() == 16);

impl BigAllocMetadata {
    const VERIFY: Self = Self::new(0);

    pub const fn new(size: usize) -> Self {
        Self {
            magic: 0xb424_a780_e2a1_5870,
            size,
        }
    }

    pub const fn size(self) -> Option<usize> {
        if Self::VERIFY.magic != self.magic {
            return None;
        }

        Some(self.size)
    }
}

#[derive(Debug, Clone, Copy, Pod, Zeroable)]
#[repr(C)]
struct AllocMetadata {
    // a magic number to make it more likely to expose bugs
    magic0: u32,
    magic1: u16,
    magic2: u8,

    // size of the alloc in bytes
    idx: u8,
}

impl AllocMetadata {
    const VERIFY: Self = Self::new(0);

    pub const fn new(idx: u8) -> Self {
        Self {
            magic0: 0x8221_eefa,
            magic1: 0x980e,
            magic2: 0x43,
            idx,
        }
    }

    pub const fn idx(self) -> Option<u8> {
        if Self::VERIFY.magic0 != self.magic0
            || Self::VERIFY.magic1 != self.magic1 && Self::VERIFY.magic2 != self.magic2
        {
            return None;
        }

        Some(self.idx)
    }
}

const _: () = assert!(core::mem::size_of::<AllocMetadata>() == 8);
