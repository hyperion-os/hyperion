#![no_std]
#![feature(const_caller_location, custom_test_frameworks, test)]

//

// use core::{ptr::NonNull, sync::atomic::AtomicUsize};

// use crossbeam::atomic::AtomicCell;

use core::sync::atomic::{AtomicBool, Ordering};

use lock_api::{Mutex, RawMutex};

// pub mod mutex;
pub mod spinlock;

//

// pub fn init_futex(wait: fn(NonNull<AtomicUsize>, usize), wake: fn(NonNull<AtomicUsize>, usize)) {
//     FUTEX_WAIT.store(wait);
//     FUTEX_WAKE.store(wake);
// }

//

// TODO: this is horrible
// static FUTEX_WAIT: AtomicCell<fn(NonNull<AtomicUsize>, usize)> =
//     AtomicCell::new(|_, _| panic!("futex not initialized"));

// static FUTEX_WAKE: AtomicCell<fn(NonNull<AtomicUsize>, usize)> =
//     AtomicCell::new(|_, _| panic!("futex not initialized"));

//

pub struct TakeOnce<T, Lock> {
    val: Mutex<Lock, Option<T>>,
    taken: AtomicBool,
}

impl<T, Lock: RawMutex> TakeOnce<T, Lock> {
    pub const fn new(val: T) -> Self {
        Self {
            val: Mutex::new(Some(val)),
            taken: AtomicBool::new(false),
        }
    }

    pub const fn none() -> Self {
        Self {
            val: Mutex::new(None),
            taken: AtomicBool::new(true),
        }
    }

    pub fn take(&self) -> Option<T> {
        if self.taken.swap(true, Ordering::AcqRel) {
            None
        } else {
            self.take_lock()
        }
    }

    #[cold]
    fn take_lock(&self) -> Option<T> {
        self.val.lock().take()
    }
}

//

#[macro_export]
macro_rules! once {
    () => {{
        use core::sync::atomic::{AtomicBool, Ordering};
        static ONCE: AtomicBool = AtomicBool::new(true);
        ONCE.swap(false, Ordering::SeqCst)
    }};
}

//

#[cfg(test)]
mod tests {}
