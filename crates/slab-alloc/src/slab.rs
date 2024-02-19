use core::{
    hint::spin_loop,
    marker::PhantomData,
    mem::{align_of, size_of},
    ptr::{addr_of_mut, null_mut, NonNull},
    sync::atomic::{AtomicPtr, Ordering},
};

use crate::{PageAlloc, SlabAllocatorStats, PAGE_SIZE};

//

// DST, `size_of::<Self>() == Slab.size`
#[repr(C)]
struct Block {
    next: AtomicPtr<Block>,
    data: [u8; 0],
}

impl Block {
    pub fn new(next: *mut Block) -> Self {
        Self {
            next: AtomicPtr::new(next),
            data: [],
        }
    }
}

//

pub struct Slab<P> {
    pub size: usize,

    head: AtomicPtr<Block>,

    _p: PhantomData<P>,
}

unsafe impl<P> Sync for Slab<P> {}
unsafe impl<P> Send for Slab<P> {}

impl<P> Slab<P> {
    #[cfg(not(all(loom, not(target_os = "none"))))]
    #[must_use]
    pub const fn new(size: usize) -> Self {
        assert!(
            size >= size_of::<u64>() && size % size_of::<u64>() == 0,
            "slab size should be a multiple of u64's size (8 bytes) and not zero"
        );

        Self {
            size,
            head: AtomicPtr::new(null_mut()),
            _p: PhantomData,
        }
    }

    #[cfg(all(loom, not(target_os = "none")))]
    #[must_use]
    pub fn new(size: usize) -> Self {
        assert!(
            size >= size_of::<u64>() && size % size_of::<u64>() == 0,
            "slab size should be a multiple of u64's size (8 bytes) and not zero"
        );

        Self {
            size,
            head: AtomicPtr::new(null_mut()),
            _p: PhantomData,
        }
    }
}

impl<P> Slab<P>
where
    P: PageAlloc,
{
    pub fn alloc(&self, idx: u8, stats: &SlabAllocatorStats) -> *mut u8 {
        #[cfg(feature = "log")]
        hyperion_log::debug!("alloc {}", self.size);

        stats.used.fetch_add(self.size, Ordering::Relaxed);
        self.pop(idx, stats).cast().as_ptr()
    }

    /// # Safety
    /// `block` must point to an allocation that was previously allocated
    /// with this specific [`Slab`]
    pub unsafe fn free(&self, stats: &SlabAllocatorStats, block: NonNull<u8>) {
        #[cfg(feature = "log")]
        hyperion_log::debug!("free {}", self.size);

        stats.used.fetch_sub(self.size, Ordering::Relaxed);
        self.push(block.cast());
    }

    fn pop(&self, idx: u8, stats: &SlabAllocatorStats) -> NonNull<Block> {
        loop {
            // fetch the head and add one to it to make it unaligned
            let head = self.head.fetch_or(0b1, Ordering::SeqCst);
            const _: () = assert!(align_of::<Block>() != 1);

            if !head.is_aligned() {
                // unaligned ptr means that another thread is currently removing an element
                while self.head.load(Ordering::SeqCst) == head {
                    spin_loop();
                }
                continue;
            }

            // aligned ptr means that the self.head is 'locked' now

            if let Some(head) = NonNull::new(head) {
                let new_head = unsafe { head.as_ref() }.next.load(Ordering::SeqCst);
                self.head.store(new_head, Ordering::SeqCst);
                return head;
            }

            // null head means that there are no elements left, so more needs to be allocated
            self.head.store(
                Self::allocate_chain(idx, self.size, stats, null_mut()),
                Ordering::SeqCst,
            );

            // retry, cuz its easier + saves a few picoseconds from other thread(s)
        }
    }

    fn push(&self, block: NonNull<Block>) {
        // let block_data: &mut [u64] =
        //     unsafe { slice::from_raw_parts_mut(block.as_ptr().cast::<u64>(), self.size / 8) };
        // block_data.fill(0); // zero out the freed memory

        // block is uninitialized
        let block_next_ptr = unsafe { addr_of_mut!((*block.as_ptr()).next) };

        loop {
            // TODO: load linked , store conditional on RISC-V and ARM
            // and cmpxchg16g on x86_64

            // this is a spinlock basically
            let old_head = self.head.fetch_or(0b1, Ordering::SeqCst);
            const _: () = assert!(align_of::<Block>() != 1);

            if !old_head.is_aligned() {
                // unaligned ptr means that another thread is currently removing an element
                while self.head.load(Ordering::SeqCst) == old_head {
                    spin_loop();
                }
                continue;
            }

            // aligned ptr means that the self.head is 'locked' now

            // atomic store would be illegal because `next` is technically uninitialized
            unsafe { block_next_ptr.write(AtomicPtr::new(old_head)) };

            let current = unsafe { old_head.byte_add(1) };
            if self
                .head
                .compare_exchange(current, block.as_ptr(), Ordering::SeqCst, Ordering::SeqCst)
                .is_ok()
            {
                break;
            }
        }
    }

    fn allocate_chain(
        idx: u8,
        size: usize,
        stats: &SlabAllocatorStats,
        next: *mut Block,
    ) -> *mut Block {
        #[cfg(feature = "log")]
        hyperion_log::debug!("alloc pages {size}");
        let page = unsafe { P::alloc(1) };
        stats.allocated.fetch_add(1, Ordering::Relaxed);

        let mut blocks = (0..PAGE_SIZE / size).map(|i| unsafe { page.first.add(i * size) });

        // the first block is the page allocation header (metadata)

        debug_assert!(size_of::<AllocMetadata>() <= size);
        let header = blocks
            .next()
            .expect("Slab size too large")
            .cast::<AllocMetadata>();
        unsafe { header.write(AllocMetadata::new(idx)) };

        // create the chain that can be pushed to the stack
        debug_assert!(size_of::<Block>() <= size);
        let mut blocks = blocks.map(|p| p.cast::<Block>());
        for (block_prev, block_next) in blocks.clone().zip(blocks.clone().skip(1)) {
            unsafe { block_prev.write(Block::new(block_next)) };
        }

        // the last block should point to the provided ptr (prob null)
        if !next.is_null() {
            let block_last = blocks.clone().last().expect("Slab size too large");
            unsafe { block_last.write(Block::new(next)) };
        }

        blocks.next().expect("Slab size too large")
    }
}

//

#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub(crate) struct BigAllocMetadata {
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

#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub(crate) struct AllocMetadata {
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
