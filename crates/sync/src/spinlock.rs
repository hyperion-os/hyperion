use core::{
    panic::Location,
    sync::atomic::{AtomicUsize, Ordering},
};

use crossbeam::atomic::AtomicCell;
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

    #[cfg(debug_assertions)]
    locked_from: AtomicCell<Option<&'static Location<'static>>>,
}

const _: () = assert!(AtomicCell::<Option<&'static Location<'static>>>::is_lock_free());

//

unsafe impl lock_api::RawMutex for SpinLock {
    #[allow(clippy::declare_interior_mutable_const)]
    const INIT: SpinLock = SpinLock {
        lock: AtomicUsize::new(UNLOCKED),

        #[cfg(debug_assertions)]
        locked_from: AtomicCell::new(None),
    };

    type GuardMarker = GuardSend;

    #[track_caller]
    fn lock(&self) {
        let id = cpu_id();

        if self.lock.load(Ordering::Relaxed) == id {
            let now = Location::caller();

            #[cfg(debug_assertions)]
            {
                let from = self.locked_from.load().unwrap();
                panic!("deadlock:\n - earlier: {from}\n - now: {now}",);
            }

            #[cfg(not(debug_assertions))]
            panic!("deadlock:\n - earlier: [debug mode needed]\n - now: {now}",);
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

        #[cfg(debug_assertions)]
        self.locked_from.store(Some(Location::caller()));
    }

    fn try_lock(&self) -> bool {
        let id = cpu_id();

        let locked = self
            .lock
            .compare_exchange(UNLOCKED, id, Ordering::Acquire, Ordering::Relaxed)
            .is_ok();

        if locked {
            #[cfg(debug_assertions)]
            self.locked_from.store(Some(Location::caller()));
        }

        locked
    }

    unsafe fn unlock(&self) {
        self.lock.store(UNLOCKED, Ordering::Release);
    }
}

impl SpinLock {
    #[track_caller]
    pub const fn new() -> Self {
        Self {
            lock: AtomicUsize::new(UNLOCKED),

            #[cfg(debug_assertions)]
            locked_from: AtomicCell::new(Some(Location::caller())),
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
