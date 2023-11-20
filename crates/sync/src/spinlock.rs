use core::sync::atomic::{AtomicUsize, Ordering};

use hyperion_cpu_id::cpu_id;
use lock_api::GuardSend;

//

const UNLOCKED: usize = usize::MAX;

//

pub type Mutex<T> = lock_api::Mutex<SpinLock, T>;
pub type MutexGuard<'a, T> = lock_api::MutexGuard<'a, SpinLock, T>;

//

pub struct SpinLock {
    // cpu id of the lock holder, usize::MAX is unlocked
    lock: AtomicUsize,
}

//

unsafe impl lock_api::RawMutex for SpinLock {
    #[allow(clippy::declare_interior_mutable_const)]
    const INIT: SpinLock = SpinLock {
        lock: AtomicUsize::new(UNLOCKED),
    };

    type GuardMarker = GuardSend;

    fn lock(&self) {
        let id = cpu_id();

        if self.lock.load(Ordering::Relaxed) == id {
            panic!("deadlock");
        }

        while self
            .lock
            .compare_exchange_weak(UNLOCKED, id, Ordering::Acquire, Ordering::Relaxed)
            .is_err()
        {
            // wait until the lock looks unlocked before retrying
            while self.lock.load(Ordering::Relaxed) != UNLOCKED {
                core::hint::spin_loop();
            }
        }
    }

    fn try_lock(&self) -> bool {
        let id = cpu_id();

        self.lock
            .compare_exchange(UNLOCKED, id, Ordering::Acquire, Ordering::Relaxed)
            .is_ok()
    }

    unsafe fn unlock(&self) {
        self.lock.store(UNLOCKED, Ordering::Release);
    }
}

impl SpinLock {
    pub const fn new() -> Self {
        Self {
            lock: AtomicUsize::new(UNLOCKED),
        }
    }
}

//

#[cfg(test)]
mod tests {
    // #[test_case]
    // fn basic_deadlock_test() {
    //     let lock = Mutex::new(5);
    //     let v1 = lock.lock();
    //     let v2 = lock.lock();
    //     let _ = v1;
    //     let _ = v2;
    // }
}
