#![no_std]
// #![feature(ptr_metadata)]

extern crate alloc;

//

use core::{
    cell::UnsafeCell,
    mem::MaybeUninit,
    sync::atomic::{AtomicBool, Ordering},
};

//

// pub mod aarc;

//

pub struct TakeOnce<T> {
    taken: AtomicBool,
    val: UnsafeCell<MaybeUninit<T>>,
}

unsafe impl<T: Send> Send for TakeOnce<T> {}
unsafe impl<T: Send> Sync for TakeOnce<T> {}

impl<T> TakeOnce<T> {
    pub const fn new(val: T) -> Self {
        Self {
            val: UnsafeCell::new(MaybeUninit::new(val)),
            taken: AtomicBool::new(false),
        }
    }

    pub const fn none() -> Self {
        Self {
            val: UnsafeCell::new(MaybeUninit::uninit()),
            taken: AtomicBool::new(true),
        }
    }

    pub fn take(&self) -> Option<T> {
        if self.taken.swap(true, Ordering::Acquire) {
            None
        } else {
            // SAFETY: exclusive access taken, the `swap` will return `false` only once
            let val_ref = unsafe { &mut *self.val.get() };

            // SAFETY: the value won't be taken after this
            let val = unsafe { val_ref.assume_init_read() };

            Some(val)
        }
    }
}

//

/// CPUs race and only the first one returns true
#[macro_export]
macro_rules! once {
    () => {{
        use core::sync::atomic::{AtomicBool, Ordering};
        static ONCE: AtomicBool = AtomicBool::new(true);
        ONCE.swap(false, Ordering::SeqCst)
    }};
}

/// CPUs race and only the last one returns true
#[macro_export]
macro_rules! last {
    () => {{
        use core::sync::atomic::{AtomicUsize, Ordering};

        use hyperion_boot::cpu_count;
        static ONCE: AtomicUsize = AtomicUsize::new(1);
        ONCE.fetch_add(1, Ordering::SeqCst) == cpu_count()
    }};
}

//

#[cfg(test)]
mod tests {}
