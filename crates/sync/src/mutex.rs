use core::{
    cell::UnsafeCell,
    marker::PhantomData,
    ops,
    panic::Location,
    sync::atomic::{AtomicUsize, Ordering},
};

use crossbeam::atomic::AtomicCell;
use hyperion_arch::cpu_id;

//

pub const UNLOCKED: usize = usize::MAX;

//

pub struct Mutex<T, W = Spin> {
    // imp: spin::Mutex<T>,
    val: UnsafeCell<T>,

    // cpu id of the lock holder, usize::MAX is unlocked
    lock: AtomicUsize,

    locked_from: AtomicCell<&'static Location<'static>>,

    _p: PhantomData<W>,
}

const _: () = assert!(AtomicCell::<&'static Location<'static>>::is_lock_free());

#[derive(Debug)]
pub enum LockError {
    /// a direct deadlock where the current cpu locked this mutex already
    Deadlock {
        /// where the lock was locked the first time
        prev: &'static Location<'static>,
    },
}

//

impl<T> Mutex<T, Spin> {
    #[track_caller]
    pub const fn new(val: T) -> Self {
        Self {
            val: UnsafeCell::new(val),
            lock: AtomicUsize::new(UNLOCKED),
            locked_from: AtomicCell::new(Location::caller()),
            _p: PhantomData,
        }
    }

    pub fn get_mut(&mut self) -> &mut T {
        self.val.get_mut()
    }

    // pub unsafe fn force_unlock(&self) {}
}

impl<T, W: Wait> Mutex<T, W> {
    #[track_caller]
    pub fn lock(&self) -> Result<MutexGuard<T, W>, LockError> {
        // basically the same as spin::Mutex::lock;

        let id = cpu_id();

        while self
            .lock
            .compare_exchange_weak(UNLOCKED, id, Ordering::Acquire, Ordering::Relaxed)
            .is_err()
        {
            // wait until the lock looks unlocked before retrying
            loop {
                let current = self.lock.load(Ordering::Relaxed);
                if current == id {
                    return Err(LockError::Deadlock {
                        prev: self.locked_from.load(),
                    });
                } else if current != UNLOCKED {
                    W::wait(&self.lock as *const _ as usize);
                } else {
                    break;
                }
            }
        }

        self.locked_from.store(Location::caller());

        Ok(MutexGuard {
            lock: &self.lock,
            val: unsafe { &mut *self.val.get() },
            _p: PhantomData,
        })
    }
}

//

pub struct MutexGuard<'a, T, W: Wait> {
    lock: &'a AtomicUsize,
    val: &'a mut T,
    _p: PhantomData<W>,
}

impl<'a, T, W: Wait> ops::Deref for MutexGuard<'a, T, W> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.val
    }
}

impl<'a, T, W: Wait> ops::DerefMut for MutexGuard<'a, T, W> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.val
    }
}

impl<'a, T, W: Wait> Drop for MutexGuard<'a, T, W> {
    fn drop(&mut self) {
        self.lock.store(UNLOCKED, Ordering::Release);
        W::wake_up(&self.lock as *const _ as usize);
    }
}

//

pub trait Wait {
    /// wait for something at this address
    fn wait(addr: usize);

    /// wake up at least at least one waiter at this address
    fn wake_up(addr: usize);
}

//

pub struct Spin;

impl Wait for Spin {
    fn wait(_addr: usize) {
        core::hint::spin_loop()
    }

    fn wake_up(_addr: usize) {}
}

//

#[cfg(test)]
mod tests {
    use super::Mutex;

    #[test_case]
    fn basic_deadlock_test() {
        let lock = Mutex::new(5);
        let v1 = lock.lock().expect("expected to lock just fine");
        let v2 = lock.lock().map(|_| ()).expect_err("expected to be Err");
        let _ = v1;
        let _ = v2;
    }
}
