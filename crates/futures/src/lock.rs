use core::{
    cell::UnsafeCell,
    ops::{Deref, DerefMut},
    sync::atomic::{AtomicBool, Ordering},
};

use event_listener::Event;

use crate::block_on;

//

pub struct Mutex<T: ?Sized> {
    lock: Lock,
    value: UnsafeCell<T>,
}

unsafe impl<T: ?Sized + Send> Sync for Mutex<T> {}
unsafe impl<T: ?Sized + Send> Send for Mutex<T> {}

impl<T> Mutex<T> {
    pub const fn new(v: T) -> Self {
        Self {
            lock: Lock::new(),
            value: UnsafeCell::new(v),
        }
    }
}

impl<T: ?Sized> Mutex<T> {
    pub const fn get_mut(&mut self) -> &mut T {
        self.value.get_mut()
    }

    pub fn is_locked(&self) -> bool {
        self.lock.is_locked()
    }

    pub fn try_lock(&self) -> Option<MutexGuard<T>> {
        if self.lock.try_lock() {
            Some(unsafe { self.guard() })
        } else {
            None
        }
    }

    pub fn lock_spin(&self) -> MutexGuard<T> {
        self.lock.lock_spin();
        unsafe { self.guard() }
    }

    pub fn lock_block(&self) -> MutexGuard<T> {
        self.lock.lock_block();
        unsafe { self.guard() }
    }

    pub async fn lock(&self) -> MutexGuard<T> {
        self.lock.lock().await;
        unsafe { self.guard() }
    }

    unsafe fn guard(&self) -> MutexGuard<T> {
        MutexGuard { mutex: self }
    }

    unsafe fn unlock(&self) {
        unsafe { self.lock.unlock() };
    }
}

//

pub struct MutexGuard<'a, T: ?Sized> {
    mutex: &'a Mutex<T>,
}

impl<'a, T: ?Sized> Deref for MutexGuard<'a, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        unsafe { &*self.mutex.value.get() }
    }
}

impl<'a, T: ?Sized> DerefMut for MutexGuard<'a, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { &mut *self.mutex.value.get() }
    }
}

impl<'a, T: ?Sized> Drop for MutexGuard<'a, T> {
    fn drop(&mut self) {
        unsafe {
            self.mutex.unlock();
        }
    }
}

//

pub struct Lock {
    state: AtomicBool,
    wakers: Event,
}

//

impl Lock {
    pub const fn new() -> Self {
        Self {
            state: AtomicBool::new(UNLOCKED),
            wakers: Event::new(),
        }
    }

    pub fn is_locked(&self) -> bool {
        self.state.load(Ordering::Acquire) == LOCKED
    }

    pub fn try_lock(&self) -> bool {
        self.state
            .compare_exchange(UNLOCKED, LOCKED, Ordering::Acquire, Ordering::Relaxed)
            .is_ok()
    }

    pub fn lock_spin(&self) {
        while self
            .state
            .compare_exchange(UNLOCKED, LOCKED, Ordering::Acquire, Ordering::Relaxed)
            .is_err()
        {
            while self.is_locked() {
                core::hint::spin_loop();
            }
        }
    }

    pub fn lock_block(&self) {
        block_on(self.lock());
    }

    // pub fn lock(&self) -> Locking<T> {
    //     Locking { mutex: Some(self) }
    // }

    pub async fn lock(&self) {
        if self.try_lock() {
            return;
        }

        self.lock_slow().await;
    }

    #[cold]
    async fn lock_slow(&self) {
        loop {
            let l = self.wakers.listen();

            if self.try_lock() {
                return;
            }

            l.await;

            if self.try_lock() {
                return;
            }
        }
    }

    /// # Safety
    /// unlocking is only safe when the MutexGuard is lost
    /// and its drop never ran, like with mem::forget
    pub unsafe fn unlock(&self) {
        self.state.store(UNLOCKED, Ordering::Release);
        self.wakers.notify(1);
    }
}

//

const UNLOCKED: bool = false;
const LOCKED: bool = true;
