#![no_std]

//

use core::sync::atomic::{AtomicBool, Ordering};

use lock_api::{Mutex, RawMutex};

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
