//! Slab allocator
//!
//! Each allocated page is divided into `n` [`Block`]s where `n = page_size / slab_size`.
//!
//! The first [`Block`] is the [`SlabHeader`], it tells the [`SlabAllocator`] which [`Slab`] allocated
//! that [`Block`].
//!
//! The other [`Block`]s are pushed into a linked list.
//!
//! The metadata like [`SlabHeader`] or [`SlabData`] is stored in the first bytes of the [`Block`].
//!
//! When a block is allocated, it is removed from the linked list and when it is freed, it is
//! pushed back into it.

//

use core::{
    slice,
    sync::atomic::{AtomicUsize, Ordering},
};

use bytemuck::{Pod, Zeroable};
use spin::RwLock;
use x86_64::VirtAddr;

use super::{
    from_higher_half,
    pmm::{self, PageFrame},
};

//

pub struct SlabAllocator {
    slabs: [(RwLock<Slab>, usize); 13],
    stats: SlabAllocatorStats,
}

#[derive(Debug)]
pub struct SlabAllocatorStats {
    /// memory given out by this slab allocator
    used: AtomicUsize,
    /// physical memory allocated by this slab allocator
    allocated: AtomicUsize,
}

pub struct Slab {
    idx: u8,
    size: usize,

    next: VirtAddr,
}

//

impl SlabAllocator {
    pub const fn new() -> Self {
        Self {
            slabs: [
                Self::new_slab(0, 8),
                Self::new_slab(1, 16),
                Self::new_slab(2, 32),
                Self::new_slab(3, 48),
                Self::new_slab(4, 64),
                Self::new_slab(5, 96),
                Self::new_slab(6, 128),
                Self::new_slab(7, 192),
                Self::new_slab(8, 256),
                Self::new_slab(9, 384),
                Self::new_slab(10, 512),
                Self::new_slab(11, 768),
                Self::new_slab(12, 1024),
            ],

            stats: SlabAllocatorStats {
                used: AtomicUsize::new(0),
                allocated: AtomicUsize::new(0),
            },
        }
    }

    pub fn get_slab(&self, size: usize) -> Option<&RwLock<Slab>> {
        self.slabs
            .iter()
            .find(|(_, slab_size)| *slab_size >= size)
            .map(|(slab, _)| slab)
    }

    pub fn alloc(&self, size: usize) -> VirtAddr {
        // crate::println!("alloc {size}");
        if let Some(slab) = self.get_slab(size) {
            slab.write().alloc(&self.stats)
        } else {
            self.big_alloc(size)
        }
    }

    /// # Safety
    /// The `v_addr` pointer must point to an allocation that was previously allocated
    /// with this specific [`SlabAllocator`]
    pub unsafe fn free(&self, v_addr: VirtAddr) {
        if v_addr.as_u64() == 0 {
            return;
        }

        if v_addr.is_aligned(0x1000u64) {
            return self.big_free(v_addr);
        }

        let page = v_addr.align_down(0x1000u64);
        let header: AllocMetadata = unsafe { *page.as_ptr() };

        let (slab, _) = header
            .idx()
            .and_then(|idx| self.slabs.get(idx as usize))
            .expect("alloc header to be valid");

        slab.write().free(&self.stats, v_addr);
    }

    const fn new_slab(idx: u8, size: usize) -> (RwLock<Slab>, usize) {
        (RwLock::new(Slab::new(idx, size)), size)
    }

    fn big_alloc(&self, size: usize) -> VirtAddr {
        // minimum number of pages for the alloc + 1 page
        // for metadata
        let pages = size.div_ceil(0x1000) + 1;
        let mut pages = pmm::PFA.alloc(pages);

        self.stats
            .allocated
            .fetch_add(pages.len(), Ordering::SeqCst);
        self.stats
            .used
            .fetch_add(pages.byte_len(), Ordering::SeqCst);

        // write the big alloc metadata
        let metadata: &mut [BigAllocMetadata] =
            bytemuck::try_cast_slice_mut(pages.as_bytes_mut()).expect("allocation to be aligned");
        metadata[0] = BigAllocMetadata::new(size as u64);

        // trace!("BigAlloc    {:#x} {size}", pages.addr().as_u64());

        // pmm already zeroed the memory
        //
        // the returned memory is the next page, because this page contains the metadata
        pages.virtual_addr() + 0x1000u64
    }

    /// # Safety
    /// The `v_addr` pointer must point to a big allocation that was previously allocated
    /// with this specific [`SlabAllocator`]
    unsafe fn big_free(&self, mut v_addr: VirtAddr) {
        v_addr -= 0x1000u64;

        let metadata: BigAllocMetadata = unsafe { *v_addr.as_ptr() };
        let size = metadata.size().expect("big alloc metadata to be valid");

        let pages = (size as usize).div_ceil(0x1000) + 1;
        let pages = unsafe { PageFrame::new(from_higher_half(v_addr), pages) };

        self.stats
            .allocated
            .fetch_sub(pages.len(), Ordering::SeqCst);
        self.stats
            .used
            .fetch_sub(pages.byte_len(), Ordering::SeqCst);

        // trace!("BigFree     {:#x} {size}", pages.addr().as_u64());

        pmm::PFA.free(pages);
    }
}

impl Default for SlabAllocator {
    fn default() -> Self {
        Self::new()
    }
}

impl Slab {
    pub const fn new(idx: u8, size: usize) -> Self {
        assert!(
            size >= core::mem::size_of::<u64>() && size % core::mem::size_of::<u64>() == 0,
            "slab size should be a multiple of u64's size (8 bytes) and not zero"
        );

        Self {
            idx,
            size,
            next: VirtAddr::new_truncate(0),
        }
    }

    pub fn next_block(&mut self, stats: &SlabAllocatorStats) -> VirtAddr {
        // hyperion_log::trace!("allocating {}", self.size);
        if !self.next.is_null() {
            // hyperion_log::trace!("using a preallocated slab");
            return self.next;
        }

        let mut page = pmm::PFA.alloc(1);
        stats.allocated.fetch_add(1, Ordering::SeqCst);
        // let page_bytes = page.byte_len();
        // let page = to_higher_half(page.addr());

        let block_parts: &mut [u64] = bytemuck::cast_slice_mut(page.as_bytes_mut());
        let mut blocks = block_parts.chunks_exact_mut(self.size / core::mem::size_of::<u64>());

        // write header

        let header = &mut blocks.next().expect("Slab size too large")[0];
        let header: &mut AllocMetadata = bytemuck::cast_mut(header);
        *header = AllocMetadata::new(self.idx);

        // create a slab chain

        let mut first = None::<VirtAddr>;
        let mut prev = None::<&mut [u64]>;
        for next in blocks {
            let addr = VirtAddr::new(next.as_ptr() as _);
            if first.is_none() {
                first = Some(addr);
            }
            if let Some(prev) = prev {
                prev[0] = addr.as_u64();
            }

            prev = Some(next);
        }

        let (Some(first), Some(prev)) = (first, prev) else {
            panic!("Slab size too large");
        };

        prev[0] = 0;

        first
    }

    pub fn alloc(&mut self, stats: &SlabAllocatorStats) -> VirtAddr {
        let block = self.next_block(stats);

        let block_data: &mut [u64] =
            unsafe { slice::from_raw_parts_mut(block.as_mut_ptr(), self.size / 8) };

        self.next = VirtAddr::new(block_data[0]);
        block_data[0] = 0; // zero out the first u64 that was used as the 'next' pointer

        stats.used.fetch_add(self.size, Ordering::SeqCst);

        block
    }

    /// # Safety
    /// The `block` pointer must point to an allocation that was previously allocated
    /// with this specific [`Slab`]
    pub unsafe fn free(&mut self, stats: &SlabAllocatorStats, block: VirtAddr) {
        let block_data: &mut [u64] =
            unsafe { slice::from_raw_parts_mut(block.as_mut_ptr(), self.size / 8) };
        block_data.fill(0);

        block_data[0] = self.next.as_u64();
        self.next = block;

        stats.used.fetch_sub(self.size, Ordering::SeqCst);
    }
}

//

#[derive(Debug, Clone, Copy, Pod, Zeroable)]
#[repr(C)]
struct BigAllocMetadata {
    // a magic number to make it more likely to expose bugs
    magic: u64,

    // size of the alloc in bytes
    size: u64,
}

const _: () = assert!(core::mem::size_of::<BigAllocMetadata>() == 16);

impl BigAllocMetadata {
    const VERIFY: Self = Self::new(0);

    pub const fn new(size: u64) -> Self {
        Self {
            magic: 0xb424_a780_e2a1_5870,
            size,
        }
    }

    pub const fn size(self) -> Option<u64> {
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
