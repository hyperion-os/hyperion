use alloc::sync::Arc;
use core::{
    cell::UnsafeCell,
    future::Future,
    mem,
    ops::{Deref, DerefMut},
    ptr,
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

    pub async fn lock_arc(self: &Arc<Self>) -> ArcMutexGuard<T> {
        self.lock.lock().await;
        unsafe { self.arc_guard() }
    }

    pub unsafe fn get_force(&self) -> &T {
        unsafe { &*self.value.get() }
    }

    pub unsafe fn get_mut_force(&self) -> &mut T {
        unsafe { &mut *self.value.get() }
    }

    unsafe fn guard(&self) -> MutexGuard<T> {
        MutexGuard {
            mutex: &self.lock,
            val: self.value.get(),
        }
    }

    unsafe fn arc_guard(self: &Arc<Self>) -> ArcMutexGuard<T> {
        ArcMutexGuard {
            mutex: self.clone(),
        }
    }

    pub unsafe fn unlock(&self) {
        unsafe { self.lock.unlock() };
    }
}

impl<T: Default> Default for Mutex<T> {
    fn default() -> Self {
        Self::new(T::default())
    }
}

//

pub struct MutexGuard<'a, T: ?Sized + 'a> {
    mutex: &'a Lock,
    val: *mut T,
}

unsafe impl<T: ?Sized + Send> Sync for MutexGuard<'_, T> {}
unsafe impl<T: ?Sized + Send> Send for MutexGuard<'_, T> {}

impl<'a, T: ?Sized + 'a> MutexGuard<'a, T> {
    pub fn map<U, Fn>(self: MutexGuard<'a, T>, f: Fn) -> MutexGuard<'a, U>
    where
        Fn: FnOnce(&mut T) -> &mut U,
    {
        let mutex = self.mutex;
        let val = f(unsafe { &mut *self.val });
        mem::forget(self);

        MutexGuard { mutex, val }
    }

    pub fn try_map<U, Fn>(self: MutexGuard<'a, T>, f: Fn) -> Option<MutexGuard<'a, U>>
    where
        Fn: FnOnce(&mut T) -> Option<&mut U>,
    {
        let mutex = self.mutex;
        let val = f(unsafe { &mut *self.val })?;
        // returning early doesnt forget the old lock yet, and drops it

        mem::forget(self);
        Some(MutexGuard { mutex, val })
    }

    pub async fn map_async<U, Fn, Fut>(self: MutexGuard<'a, T>, f: Fn) -> MutexGuard<'a, U>
    where
        Fn: FnOnce(&'a mut T) -> Fut,
        Fut: Future<Output = &'a mut U>,
        U: 'a,
    {
        let mutex = self.mutex;
        let val = f(unsafe { &mut *self.val }).await;
        mem::forget(self);

        MutexGuard { mutex, val }
    }
}

impl<T: ?Sized> Deref for MutexGuard<'_, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        unsafe { &*self.val }
    }
}

impl<T: ?Sized> DerefMut for MutexGuard<'_, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { &mut *self.val }
    }
}

impl<T: ?Sized> Drop for MutexGuard<'_, T> {
    fn drop(&mut self) {
        unsafe {
            self.mutex.unlock();
        }
    }
}

//

pub struct ArcMutexGuard<T: ?Sized> {
    mutex: Arc<Mutex<T>>,
}

impl<T: ?Sized> Deref for ArcMutexGuard<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        unsafe { self.mutex.get_force() }
    }
}

impl<T: ?Sized> DerefMut for ArcMutexGuard<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { self.mutex.get_mut_force() }
    }
}

impl<T: ?Sized> Drop for ArcMutexGuard<T> {
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

impl Default for Lock {
    fn default() -> Self {
        Self::new()
    }
}

//

const UNLOCKED: bool = false;
const LOCKED: bool = true;
