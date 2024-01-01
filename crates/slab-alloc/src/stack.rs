use core::{
    mem::align_of,
    ptr::NonNull,
    sync::atomic::{AtomicPtr, Ordering},
};

use crate::SlabAllocatorStats;

//

pub struct Stack {
    head: AtomicPtr<Block>,
}

impl Stack {
    fn pop(&self, idx: u8, stats: &SlabAllocatorStats) -> NonNull<Block> {
        loop {
            // fetch the head and add one to it to make it unaligned
            let head = self.head.fetch_or(0b1, Ordering::SeqCst);

            const _: () = assert_ne!(align_of::<Block>(), 1);

            if !head.is_aligned() {
                // unaligned ptr means that another thread is currently removing an element
                while self.head.load(Ordering::SeqCst) == head {}
                continue;
            }

            // aligned ptr means that the self.head is 'locked' now

            if let Some(head) = NonNull::new(head) {
                let new_head = unsafe { head.as_ref() }.next.load(Ordering::SeqCst);
                self.head.store(new_head, Ordering::SeqCst);
                break head;
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
        let block_data: &mut [u64] =
            unsafe { slice::from_raw_parts_mut(block.as_ptr().cast::<u64>(), self.size / 8) };
        // block_data.fill(0); // zero out the freed memory

        // block is uninitialized
        let block_next_ptr = unsafe { addr_of_mut!((*block.as_ptr()).next) };

        loop {
            let old_head = self.head.load(Ordering::SeqCst);

            if !old_head.is_aligned() {
                // unaligned ptr means that another thread is currently removing an element
                while self.head.load(Ordering::SeqCst) == old_head {}
                continue;
            }

            // atomic store would be illegal because `next` is technically uninitialized
            unsafe { block_next_ptr.write(AtomicPtr::new(old_head)) };

            if self
                .head
                .compare_exchange(old_head, block.as_ptr(), Ordering::SeqCst, Ordering::SeqCst)
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
        let mut page = P::alloc(1);
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
        let blocks = blocks.map(|p| p.cast::<Block>());
        for (block_prev, block_next) in blocks.clone().zip(blocks.clone().skip(1)) {
            unsafe { block_prev.write(Block::new(block_next)) };
        }

        // the last block should point to the provided ptr (prob null)
        let block_last = blocks.last().expect("Slab size too large");
        unsafe { block_last.write(Block::new(next)) };

        block_last
    }
}

//

// DST, `size_of::<Self>() == Slab.size`
#[repr(C)]
pub(crate) struct Block {
    next: AtomicPtr<Block>,
    data: [u8; 0],
}

impl Block {
    pub const fn new(next: *mut Block) -> Self {
        Self {
            next: AtomicPtr::new(next),
            data: [],
        }
    }
}
