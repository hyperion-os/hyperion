use core::{
    alloc::Layout,
    mem::MaybeUninit,
    ptr::{self, NonNull},
};

use crate::SlabAllocator;

//

pub struct LocalAllocator {
    slabs: [LocalSlab; 13],
}

impl LocalAllocator {
    pub const fn new() -> Self {
        Self {
            slabs: [const { LocalSlab::new() }; 13],
        }
    }

    pub fn alloc<'a, P, Lock>(
        &'a mut self,
        alloc: &'a SlabAllocator<P, Lock>,
        layout: Layout,
    ) -> *mut u8 {
        let size = layout.size(); // TODO: align_up(size, align)

        todo!()
        /* match size {
            0 => ptr::null_mut(),
            1..=8 => {
                if let Some(alloc) = self.slabs[0].alloc() {
                    return alloc;
                } else {
                    // refill 50%
                    self.slabs[0];
                }
            }
            9..=16 => {}
            17..=32 => {}
            33..=48 => {}
            49..=64 => {}
            65..=96 => {}
            97..=128 => {}
            129..=192 => {}
            193..=256 => {}
            257..=384 => {}
            385..=512 => {}
            513..=768 => {}
            769..=1024 => {}
            1025.. => {}
        } */
    }

    pub fn dealloc<'a, P, Lock>(&'a mut self, alloc: &'a SlabAllocator<P, Lock>, ptr: NonNull<u8>) {
        todo!()
    }
}

struct LocalSlab {
    // 37 and 13 happen to keep the size size of the LocalAllocator just under one page
    // on x86_64
    buffer: StaticVec<*mut u8, 37>,
}

impl LocalSlab {
    const fn new() -> Self {
        Self {
            buffer: StaticVec::new(),
        }
    }

    fn alloc(&mut self) -> Option<*mut u8> {
        self.buffer.pop()
    }

    fn dealloc(&mut self, alloc: *mut u8) -> Result<(), *mut u8> {
        self.buffer.push(alloc)
    }
}

struct StaticVec<T, const CAP: usize> {
    inner: [MaybeUninit<T>; CAP],
    len: usize,
}

impl<T, const CAP: usize> StaticVec<T, CAP> {
    const fn new() -> Self {
        Self {
            inner: [const { MaybeUninit::uninit() }; CAP],
            len: 0,
        }
    }

    fn push(&mut self, val: T) -> Result<(), T> {
        if self.len == CAP {
            return Err(val);
        }
        self.len += 1;
        self.inner[self.len - 1].write(val);
        Ok(())
    }

    fn pop(&mut self) -> Option<T> {
        if self.len == 0 {
            return None;
        }
        self.len -= 1;
        Some(unsafe { self.inner[self.len].assume_init_read() })
    }
}
