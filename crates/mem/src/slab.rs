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

use spin::RwLock;
use volatile::Volatile;
use x86_64::VirtAddr;

use super::{
    from_higher_half,
    pmm::{self, PageFrame},
    to_higher_half,
};

//

pub struct SlabAllocator {
    slabs: [(RwLock<Slab>, usize); 11],
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

#[derive(Debug, Clone, Copy)]
#[repr(transparent)]
pub struct BigAllocPageMetadata {
    // size of the alloc in bytes
    size: usize,
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

    pub fn free(&self, v_addr: VirtAddr) {
        if v_addr.as_u64() == 0 {
            return;
        }

        if v_addr.is_aligned(0x1000u64) {
            return self.big_free(v_addr);
        }

        let page = v_addr.align_down(0x1000u64);
        let header: &mut u8 = unsafe { &mut *page.as_mut_ptr() };

        let (slab, _) = &self.slabs[*header as usize];
        slab.write().free(&self.stats, v_addr);
    }

    const fn new_slab(idx: u8, size: usize) -> (RwLock<Slab>, usize) {
        (RwLock::new(Slab::new(idx, size)), size)
    }

    fn big_alloc(&self, size: usize) -> VirtAddr {
        // minimum number of pages for the alloc + 1 page
        // for metadata
        let pages = size.div_ceil(0x1000) + 1;
        let mut pages = pmm::PageFrameAllocator::get().alloc(pages);

        self.stats
            .allocated
            .fetch_add(pages.len(), Ordering::SeqCst);
        self.stats
            .used
            .fetch_add(pages.byte_len(), Ordering::SeqCst);

        // write the big alloc metadata
        let metadata: &mut [BigAllocPageMetadata] = pages.as_mut_slice();
        Volatile::new_write_only(&mut metadata[0]).write(BigAllocPageMetadata { size });

        // trace!("BigAlloc    {:#x} {size}", pages.addr().as_u64());

        // pmm already zeroed the memory
        //
        // the returned memory is the next page, because this page contains the metadata
        to_higher_half(pages.physical_addr()) + 0x1000u64
    }

    fn big_free(&self, mut v_addr: VirtAddr) {
        // TODO: what if v_addr is invalid?

        v_addr -= 0x1000u64;

        let metadata: &BigAllocPageMetadata = unsafe { &*v_addr.as_ptr() };
        let size = Volatile::new_read_only(&metadata).read().size;

        let pages = size.div_ceil(0x1000) + 1;
        let pages = unsafe { PageFrame::new(from_higher_half(v_addr), pages) };

        self.stats
            .allocated
            .fetch_sub(pages.len(), Ordering::SeqCst);
        self.stats
            .used
            .fetch_sub(pages.byte_len(), Ordering::SeqCst);

        // trace!("BigFree     {:#x} {size}", pages.addr().as_u64());

        pmm::PageFrameAllocator::get().free(pages);
    }
}

impl Default for SlabAllocator {
    fn default() -> Self {
        Self::new()
    }
}

impl Slab {
    pub const fn new(idx: u8, size: usize) -> Self {
        Self {
            idx,
            size,
            next: VirtAddr::new_truncate(0),
        }
    }

    pub fn next_block(&mut self, stats: &SlabAllocatorStats) -> VirtAddr {
        if !self.next.is_null() {
            return self.next;
        }

        let mut page = pmm::PageFrameAllocator::get().alloc(1);
        stats.allocated.fetch_add(1, Ordering::SeqCst);
        // let page_bytes = page.byte_len();
        // let page = to_higher_half(page.addr());

        let mut blocks = page.as_mut_slice().chunks_exact_mut(self.size / 8);

        // write header

        let header = blocks.next().expect("Slab size too large");
        header[0] = self.idx as u64;

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

    pub fn free(&mut self, stats: &SlabAllocatorStats, block: VirtAddr) {
        let block_data: &mut [u64] =
            unsafe { slice::from_raw_parts_mut(block.as_mut_ptr(), self.size / 8) };

        block_data[0] = self.next.as_u64();
        self.next = block;

        stats.used.fetch_sub(self.size, Ordering::SeqCst);
    }
}
