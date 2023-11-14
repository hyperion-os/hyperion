#[cfg(not(debug_assertions))]
use core::cell::UnsafeCell;
use core::{
    fmt,
    ops::{Deref, DerefMut},
    ptr::NonNull,
    sync::atomic::{AtomicUsize, Ordering},
};

use crate::{spinlock, FUTEX_WAIT, FUTEX_WAKE};

//

const LOCKED: usize = 1;
const UNLOCKED: usize = 0;

//

pub struct Futex {
    lock: AtomicUsize,
}

impl Futex {
    pub const fn new(val: T) -> Self {
        Self {
            lock: AtomicUsize::new(UNLOCKED),
        }
    }
}

unsafe impl lock_api::RawMutex for Futex {
    fn lock(&self) -> MutexGuard<T> {
        while self
            .lock
            .compare_exchange_weak(UNLOCKED, LOCKED, Ordering::Acquire, Ordering::Relaxed)
            .is_err()
        {
            // wait until the lock looks unlocked before retrying
            let addr = NonNull::from(&self.lock);
            FUTEX_WAIT.load()(addr, LOCKED);
            // futex::wait(addr, LOCKED);
        }

        MutexGuard {
            lock: &self.lock,

            #[cfg(not(debug_assertions))]
            val: unsafe { &mut *self.val.get() },

            #[cfg(debug_assertions)]
            val: self
                .val
                .try_lock()
                .expect("should have exclusive access to self.val"),
        }
    }
}

unsafe impl<T: ?Sized + Send> Sync for Mutex<T> {}
unsafe impl<T: ?Sized + Send> Send for Mutex<T> {}

//

pub struct MutexGuard<'a, T: ?Sized> {
    lock: &'a AtomicUsize,

    #[cfg(not(debug_assertions))]
    val: &'a mut T,

    #[cfg(debug_assertions)]
    val: spinlock::MutexGuard<'a, T>,
}

impl<'a, T: fmt::Debug + ?Sized> fmt::Debug for MutexGuard<'a, T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.val.fmt(f)
    }
}

impl<'a, T: ?Sized> Deref for MutexGuard<'a, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        #[cfg(not(debug_assertions))]
        return self.val;

        #[cfg(debug_assertions)]
        &self.val
    }
}

impl<'a, T: ?Sized> DerefMut for MutexGuard<'a, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        #[cfg(not(debug_assertions))]
        return self.val;

        #[cfg(debug_assertions)]
        &mut self.val
    }
}

impl<'a, T: ?Sized> Drop for MutexGuard<'a, T> {
    fn drop(&mut self) {
        // unlock the mutex
        self.lock.store(UNLOCKED, Ordering::Release);

        // and THEN wake up waiting threads
        let addr = NonNull::from(self.lock);
        FUTEX_WAKE.load()(addr, 1);
        // futex::wake(addr, 1);
    }
}
