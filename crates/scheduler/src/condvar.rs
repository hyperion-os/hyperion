use core::{
    sync::atomic::{AtomicUsize, Ordering},
    time::Duration,
};

use lock_api::{MutexGuard, RawMutex};

use crate::{futex, lock::Futex};

//

// copied from rust std and modified to work with my futex:

pub struct Condvar {
    // The value of this atomic is simply incremented on every notification.
    // This is used by `.wait()` to not miss any notifications after
    // unlocking the mutex and before waiting for notifications.
    futex: AtomicUsize,
}

impl Condvar {
    #[inline]
    pub const fn new() -> Self {
        Self {
            futex: AtomicUsize::new(0),
        }
    }

    // All the memory orderings here are `Relaxed`,
    // because synchronization is done by unlocking and locking the mutex.

    pub fn notify_one(&self) {
        self.futex.fetch_add(1, Ordering::Relaxed);
        futex::wake(&self.futex, 1)
    }

    pub fn notify_all(&self) {
        self.futex.fetch_add(1, Ordering::Relaxed);
        futex::wake(&self.futex, usize::MAX)
    }

    pub fn wait<'a, T>(&self, mutex: MutexGuard<'a, Futex, T>) -> MutexGuard<'a, Futex, T> {
        unsafe {
            self._wait(MutexGuard::mutex(&mutex).raw());
        }

        mutex
    }

    unsafe fn _wait(&self, mutex: &Futex) {
        self.wait_optional_timeout(mutex, None);
    }

    unsafe fn _wait_timeout(&self, mutex: &Futex, timeout: Duration) -> bool {
        self.wait_optional_timeout(mutex, Some(timeout))
    }

    unsafe fn wait_optional_timeout(&self, mutex: &Futex, timeout: Option<Duration>) -> bool {
        // Examine the notification counter _before_ we unlock the mutex.
        let futex_value = self.futex.load(Ordering::Relaxed);

        // Unlock the mutex before going to sleep.
        unsafe { mutex.unlock() };

        // Wait, but only if there hasn't been any
        // notification since we unlocked the mutex.
        // let r = ..
        futex::wait(&self.futex, futex_value); // TODO: timeout

        _ = timeout;
        let r = false;

        // Lock the mutex again.
        mutex.lock();

        r
    }
}
