use core::{
    sync::atomic::{AtomicUsize, Ordering},
    time::Duration,
};

use hyperion_syscall::{futex_wait, futex_wake};
use lock_api::{GuardSend, RawMutex};

//

pub type Mutex<T> = lock_api::Mutex<Futex, T>;
pub type MutexGuard<'a, T> = lock_api::MutexGuard<'a, Futex, T>;

//

pub struct Futex {
    futex: AtomicUsize,
}

impl Futex {
    #[cold]
    fn lock_slow(&self) {
        while self
            .futex
            .compare_exchange(UNLOCKED, LOCKED, Ordering::Acquire, Ordering::Relaxed)
            .is_err()
        {
            _ = self.futex.compare_exchange(
                LOCKED,
                LOCKED_CONTENDED,
                Ordering::Acquire,
                Ordering::Relaxed,
            );

            futex_wait(&self.futex, LOCKED_CONTENDED);
        }
    }
}

unsafe impl RawMutex for Futex {
    #[allow(clippy::declare_interior_mutable_const)]
    const INIT: Self = Futex {
        futex: AtomicUsize::new(UNLOCKED),
    };

    type GuardMarker = GuardSend;

    fn lock(&self) {
        if !self.try_lock() {
            self.lock_slow();
        }
    }

    fn try_lock(&self) -> bool {
        self.futex
            .compare_exchange(UNLOCKED, LOCKED, Ordering::Acquire, Ordering::Relaxed)
            .is_ok()
    }

    unsafe fn unlock(&self) {
        // unlock the mutex
        let old = self.futex.swap(UNLOCKED, Ordering::Release);

        if old == LOCKED_CONTENDED {
            // and THEN wake up waiting threads
            futex_wake(&self.futex, 1);
        }
    }
}

//

const UNLOCKED: usize = 0;
const LOCKED: usize = 1;
const LOCKED_CONTENDED: usize = 2;

//

pub struct Condvar {
    // The value of this atomic is simply incremented on every notification.
    // This is used by `.wait()` to not miss any notifications after
    // unlocking the mutex and before waiting for notifications.
    futex: AtomicUsize,
}

impl Default for Condvar {
    fn default() -> Self {
        Self::new()
    }
}

impl Condvar {
    #[inline]
    #[must_use]
    pub const fn new() -> Self {
        Self {
            futex: AtomicUsize::new(0),
        }
    }

    // All the memory orderings here are `Relaxed`,
    // because synchronization is done by unlocking and locking the mutex.

    pub fn notify_one(&self) {
        self.futex.fetch_add(1, Ordering::Relaxed);
        futex_wake(&self.futex, 1);
    }

    pub fn notify_all(&self) {
        self.futex.fetch_add(1, Ordering::Relaxed);
        futex_wake(&self.futex, usize::MAX);
    }

    pub fn wait<'a, T>(&self, mutex: MutexGuard<'a, T>) -> MutexGuard<'a, T> {
        unsafe {
            self._wait(MutexGuard::mutex(&mutex).raw());
        }

        mutex
    }

    unsafe fn _wait(&self, mutex: &Futex) {
        unsafe { self.wait_optional_timeout(mutex, None) };
    }

    unsafe fn _wait_timeout(&self, mutex: &Futex, timeout: Duration) -> bool {
        unsafe { self.wait_optional_timeout(mutex, Some(timeout)) }
    }

    unsafe fn wait_optional_timeout(&self, mutex: &Futex, timeout: Option<Duration>) -> bool {
        // Examine the notification counter _before_ we unlock the mutex.
        let futex_value = self.futex.load(Ordering::Relaxed);

        // Unlock the mutex before going to sleep.
        unsafe { mutex.unlock() };

        // Wait, but only if there hasn't been any
        // notification since we unlocked the mutex.
        // let r = ..
        futex_wait(&self.futex, futex_value); // TODO: timeout

        _ = timeout;
        let r = false;

        // Lock the mutex again.
        mutex.lock();

        r
    }
}
