use core::{
    cell::UnsafeCell,
    ops,
    panic::Location,
    sync::atomic::{AtomicUsize, Ordering},
};

use crossbeam::atomic::AtomicCell;
use hyperion_cpu_id::cpu_id;

//

pub const UNLOCKED: usize = usize::MAX;

//

pub struct Mutex<T: ?Sized> {
    // cpu id of the lock holder, usize::MAX is unlocked
    lock: AtomicUsize,

    #[cfg(debug_assertions)]
    locked_from: AtomicCell<&'static Location<'static>>,

    // imp: spin::Mutex<T>,
    val: UnsafeCell<T>,
}

const _: () = assert!(AtomicCell::<&'static Location<'static>>::is_lock_free());

//

impl<T> Mutex<T> {
    #[track_caller]
    pub const fn new(val: T) -> Self {
        Self {
            val: UnsafeCell::new(val),
            lock: AtomicUsize::new(UNLOCKED),

            #[cfg(debug_assertions)]
            locked_from: AtomicCell::new(Location::caller()),
        }
    }
}

impl<T: ?Sized> Mutex<T> {
    pub fn get_mut(&mut self) -> &mut T {
        self.val.get_mut()
    }

    // pub unsafe fn force_unlock(&self) {}

    #[track_caller]
    pub fn lock(&self) -> MutexGuard<T> {
        // basically the same as spin::Mutex::lock;

        let id = cpu_id();

        if self.lock.load(Ordering::Relaxed) == id {
            #[cfg(debug_assertions)]
            panic!(
                "deadlock:\n - earlier: {}\n - now: {}",
                self.locked_from.load(),
                core::panic::Location::caller()
            );
            #[cfg(not(debug_assertions))]
            panic!(
                "deadlock:\n - earlier: [debug mode needed]\n - now: {}",
                Location::caller()
            );
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
        self.locked_from.store(Location::caller());

        MutexGuard {
            lock: &self.lock,
            val: unsafe { &mut *self.val.get() },
        }
    }

    pub fn try_lock(&self) -> Option<MutexGuard<T>> {
        let id = cpu_id();

        if self
            .lock
            .compare_exchange(UNLOCKED, id, Ordering::Acquire, Ordering::Relaxed)
            .is_ok()
        {
            Some(MutexGuard {
                lock: &self.lock,
                val: unsafe { &mut *self.val.get() },
            })
        } else {
            None
        }
    }
}

impl<T> From<T> for Mutex<T> {
    fn from(value: T) -> Self {
        Self::new(value)
    }
}

unsafe impl<T: ?Sized + Send> Sync for Mutex<T> {}
unsafe impl<T: ?Sized + Send> Send for Mutex<T> {}

//

pub struct MutexGuard<'a, T: ?Sized> {
    lock: &'a AtomicUsize,
    val: &'a mut T,
}

impl<'a, T: ?Sized> ops::Deref for MutexGuard<'a, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.val
    }
}

impl<'a, T: ?Sized> ops::DerefMut for MutexGuard<'a, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.val
    }
}

impl<'a, T: ?Sized> Drop for MutexGuard<'a, T> {
    fn drop(&mut self) {
        self.lock.store(UNLOCKED, Ordering::Release);
    }
}

//

#[cfg(test)]
mod tests {
    use super::Mutex;

    #[test_case]
    fn basic_deadlock_test() {
        let lock = Mutex::new(5);
        let v1 = lock.lock();
        let v2 = lock.lock();
        let _ = v1;
        let _ = v2;
    }
}
