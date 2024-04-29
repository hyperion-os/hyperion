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
#![feature(
    pointer_is_aligned_to,
    const_pointer_is_aligned,
    strict_provenance_atomic_ptr,
    const_ptr_is_null,
    allocator_api,
    inline_const
)]

//

pub use alloc::{SlabAllocator, SlabAllocatorStats};
use core::slice;

pub use slab::Slab;

//

mod alloc;
// mod local;
mod slab;
// mod stack;

//

const PAGE_SIZE: usize = 0x1000; // 4KiB pages

//

/// a backend allocator, allocates whole pages (4KiB blocks)
///
/// # Safety
/// todo
pub unsafe trait PageAlloc {
    /// # Safety
    /// the returned pages should always have the requested size,
    /// the pages must be R/W and they will be exclusively owned by the slab allocator
    /// the pages should be aligned to 0x1000
    unsafe fn alloc(pages: usize) -> Pages;

    /// # Safety
    /// the pages are now in an undefined state
    unsafe fn dealloc(frames: Pages);
}

//

pub struct Pages {
    first: *mut u8,
    len: usize,
}

impl Pages {
    /// # Safety
    /// `first` must point to a valid page allocation of `len * 0x1000` bytes
    pub const unsafe fn new(first: *mut u8, len: usize) -> Self {
        debug_assert!(first.is_aligned_to(0x1000));
        Self { first, len }
    }

    #[must_use]
    pub const fn len(&self) -> usize {
        self.len
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    #[must_use]
    pub const fn byte_len(&self) -> usize {
        self.len() * PAGE_SIZE
    }

    #[must_use]
    pub const fn as_bytes(&self) -> &[u8] {
        unsafe { slice::from_raw_parts(self.first.cast_const(), self.byte_len()) }
    }

    pub fn as_bytes_mut(&mut self) -> &mut [u8] {
        unsafe { slice::from_raw_parts_mut(self.first, self.byte_len()) }
    }

    #[must_use]
    pub const fn as_ptr(&self) -> *mut u8 {
        self.first
    }
}
