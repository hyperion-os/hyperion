use core::{
    cell::Cell,
    hint::spin_loop,
    ops::Deref,
    sync::atomic::{AtomicUsize, Ordering},
};

use lock_api::GuardSend;

use crate::{futex, running};

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
    #[allow(clippy::declare_interior_mutable_const)]
    const INIT: Self = Self::new();

    type GuardMarker = GuardSend;

    fn lock(&self) {
        while self
            .lock
            .compare_exchange(UNLOCKED, LOCKED, Ordering::Acquire, Ordering::Relaxed)
            .is_err()
        {
            // wait until the lock looks unlocked before retrying
            futex::wait(&self.lock, LOCKED);
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
        futex::wake(&self.lock, 1);
    }
}

//

pub struct AutoFutex {
    lock: AtomicUsize,
}

impl AutoFutex {
    pub const fn new() -> Self {
        Self {
            lock: AtomicUsize::new(UNLOCKED),
        }
    }
}

unsafe impl lock_api::RawMutex for AutoFutex {
    #[allow(clippy::declare_interior_mutable_const)]
    const INIT: Self = Self::new();

    type GuardMarker = GuardSend;

    fn lock(&self) {
        while self
            .lock
            .compare_exchange(UNLOCKED, LOCKED, Ordering::Acquire, Ordering::Relaxed)
            .is_err()
        {
            if !running() {
                spin_loop();
                continue;
            }

            // wait until the lock looks unlocked before retrying
            futex::wait(&self.lock, LOCKED);
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

        if !running() {
            return;
        }

        // and THEN wake up waiting threads
        futex::wake(&self.lock, 1);
    }
}

//

const INIT: usize = 1;
const UNINIT: usize = 0;

//

pub struct Once<T> {
    futex: AtomicUsize,
    inner: spin::Once<T>,
}

unsafe impl<T: Send + Sync> Sync for Once<T> {}
unsafe impl<T: Send> Send for Once<T> {}

impl<T> Once<T> {
    pub const fn new() -> Self {
        Self {
            futex: AtomicUsize::new(UNINIT),
            inner: spin::Once::new(),
        }
    }

    pub const fn initialized(v: T) -> Self {
        Self {
            futex: AtomicUsize::new(UNINIT),
            inner: spin::Once::initialized(v),
        }
    }

    pub fn get(&self) -> Option<&T> {
        self.inner.get()
    }

    pub fn wait(&self) -> &T {
        loop {
            if let Some(v) = self.inner.get() {
                return v;
            }

            if !running() {
                return self.inner.wait(); // spin
            }

            futex::wait(&self.futex, UNINIT);
        }
    }

    pub fn call_once(&self, f: impl FnOnce() -> T) -> &T {
        // if let Err(old) = self
        //     .futex
        //     .compare_exchange(UNINIT, RUNNING, Ordering::Acquire, Ordering::Relaxed)
        //     .is_err()
        // {}

        self.get().unwrap_or_else(|| self.call_once_cold(f))
    }

    #[cold]
    fn call_once_cold(&self, f: impl FnOnce() -> T) -> &T {
        let mut ran = false;
        let v = self.inner.call_once(|| {
            ran = true;
            f()
        });
        if ran && running() {
            self.futex.store(INIT, Ordering::Release);
            futex::wake(&self.futex, usize::MAX);
        }
        v
    }
}

//

pub struct Lazy<T, F = fn() -> T> {
    once: Once<T>,
    init: Cell<Option<F>>,
}

unsafe impl<T, F: Send> Sync for Lazy<T, F> where Once<T>: Sync {}

impl<T, F: FnOnce() -> T> Lazy<T, F> {
    pub const fn new(f: F) -> Self {
        Self {
            once: Once::new(),
            init: Cell::new(Some(f)),
        }
    }

    pub const fn initialized(v: T) -> Self {
        Self {
            once: Once::initialized(v),
            init: Cell::new(None),
        }
    }

    pub fn force(this: &Self) -> &T {
        // based on spin::Lazy::force;
        // spin::Lazy;
        // spin::Once;
        this.once.call_once(|| match this.init.take() {
            Some(f) => f(),
            None => panic!("Lazy instance has previously been poisoned"),
        })
    }
}

impl<T, F: FnOnce() -> T> Deref for Lazy<T, F> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        Self::force(self)
    }
}
