use core::{
    ptr::NonNull,
    sync::atomic::{AtomicUsize, Ordering},
};

use lock_api::GuardSend;

use crate::futex;

//

const LOCKED: usize = 1;
const UNLOCKED: usize = 0;

//

pub type Mutex<T> = lock_api::Mutex<Futex, T>;
pub type MutexGuard<'a, T> = lock_api::MutexGuard<'a, Futex, T>;

//

pub struct Futex {
    lock: AtomicUsize,
}

impl Futex {
    pub const fn new() -> Self {
        Self {
            lock: AtomicUsize::new(UNLOCKED),
        }
    }
}

unsafe impl lock_api::RawMutex for Futex {
    const INIT: Self = Self::new();

    type GuardMarker = GuardSend;

    fn lock(&self) {
        while self
            .lock
            .compare_exchange_weak(UNLOCKED, LOCKED, Ordering::Acquire, Ordering::Relaxed)
            .is_err()
        {
            // wait until the lock looks unlocked before retrying
            let addr = NonNull::from(&self.lock);
            futex::wait(addr, LOCKED);
        }
    }

    fn try_lock(&self) -> bool {
        self.lock
            .compare_exchange_weak(UNLOCKED, LOCKED, Ordering::Acquire, Ordering::Relaxed)
            .is_ok()
    }

    unsafe fn unlock(&self) {
        // unlock the mutex
        self.lock.store(UNLOCKED, Ordering::Release);

        // and THEN wake up waiting threads
        let addr = NonNull::from(&self.lock);
        futex::wake(addr, 1);
    }
}
