//! loosely based on [`spin::RwLock`]

use core::{
    cell::UnsafeCell,
    ops::{Deref, DerefMut},
    sync::atomic::{AtomicUsize, Ordering},
};

use event_listener::Event;

//

pub struct RwLock<T: ?Sized> {
    lock: Lock,
    val: UnsafeCell<T>,
}

unsafe impl<T: ?Sized + Send> Sync for RwLock<T> {}
unsafe impl<T: ?Sized + Send + Sync> Send for RwLock<T> {}

impl<T> RwLock<T> {
    pub const fn new(val: T) -> Self {
        Self {
            val: UnsafeCell::new(val),
            lock: Lock::new(),
        }
    }
}

impl<T: ?Sized> RwLock<T> {
    pub async fn read(&self) -> RwLockReadGuard<T> {
        self.lock.read().await;
        unsafe { self.read_guard() }
    }

    pub fn try_read(&self) -> Option<RwLockReadGuard<T>> {
        if self.lock.try_read() {
            Some(unsafe { self.read_guard() })
        } else {
            None
        }
    }

    pub async fn write(&self) -> RwLockWriteGuard<T> {
        self.lock.write().await;
        unsafe { self.write_guard() }
    }

    pub fn try_write(&self) -> Option<RwLockWriteGuard<T>> {
        if self.lock.try_write() {
            Some(unsafe { self.write_guard() })
        } else {
            None
        }
    }

    pub fn get(&self) -> *mut T {
        self.val.get()
    }

    unsafe fn read_guard(&self) -> RwLockReadGuard<T> {
        RwLockReadGuard {
            lock: &self.lock,
            val: self.val.get(),
        }
    }

    unsafe fn write_guard(&self) -> RwLockWriteGuard<T> {
        RwLockWriteGuard {
            lock: &self.lock,
            val: self.val.get(),
        }
    }

    // unsafe fn arc_guard(self: &Arc<Self>) -> ArcMutexGuard<T> {
    //     ArcMutexGuard {
    //         mutex: self.clone(),
    //     }
    // }

    /// # Safety
    /// very unsafe
    ///
    /// the lock has to be unlocked/read locked AND the read guard has to be forgotten
    pub unsafe fn read_unlock(&self) {
        unsafe { self.lock.read_unlock() };
    }

    /// # Safety
    /// extremely unsafe
    ///
    /// the lock has to be unlocked/write locked AND the write guard has to be forgotten
    pub unsafe fn write_unlock(&self) {
        unsafe { self.lock.write_unlock() };
    }
}

impl<T: Default> Default for RwLock<T> {
    fn default() -> Self {
        Self::new(T::default())
    }
}

//

pub struct RwLockReadGuard<'a, T: ?Sized> {
    lock: &'a Lock,
    val: *const T,
}

unsafe impl<T: ?Sized + Send + Sync> Sync for RwLockReadGuard<'_, T> {}
unsafe impl<T: ?Sized + Send + Sync> Send for RwLockReadGuard<'_, T> {}

impl<T: ?Sized> Drop for RwLockReadGuard<'_, T> {
    fn drop(&mut self) {
        unsafe { self.lock.read_unlock() };
    }
}

impl<T: ?Sized> Deref for RwLockReadGuard<'_, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        unsafe { &*self.val }
    }
}

pub struct RwLockWriteGuard<'a, T: ?Sized> {
    lock: &'a Lock,
    val: *mut T,
}

unsafe impl<T: ?Sized + Send + Sync> Sync for RwLockWriteGuard<'_, T> {}
unsafe impl<T: ?Sized + Send + Sync> Send for RwLockWriteGuard<'_, T> {}

impl<T: ?Sized> Deref for RwLockWriteGuard<'_, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        unsafe { &*self.val }
    }
}

impl<T: ?Sized> DerefMut for RwLockWriteGuard<'_, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { &mut *self.val }
    }
}

impl<T: ?Sized> Drop for RwLockWriteGuard<'_, T> {
    fn drop(&mut self) {
        unsafe { self.lock.write_unlock() };
    }
}

//

pub struct Lock {
    state: AtomicUsize,
    readers: Event,
    writers: Event,
}

impl Lock {
    pub const fn new() -> Self {
        Self {
            state: AtomicUsize::new(UNLOCKED),
            readers: Event::new(),
            writers: Event::new(),
        }
    }

    pub fn try_read(&self) -> bool {
        let value = self.state.fetch_add(READER, Ordering::Acquire);

        if value >= MAX_READERS {
            self.state.fetch_sub(READER, Ordering::Acquire);
            panic!("too many readers");
        }

        if value & (WRITER | UPGRADE) != 0 {
            self.state.fetch_sub(READER, Ordering::Acquire);
            return false;
        }

        true
    }

    pub async fn read(&self) {
        if self.try_read() {
            return;
        }

        self.read_slow().await;
    }

    /// # Safety
    /// unlocking is only safe when the MutexGuard is lost
    /// and its drop never ran, like with mem::forget
    pub unsafe fn read_unlock(&self) {
        self.state.fetch_sub(READER, Ordering::Release);
        self.writers.notify(1);
    }

    #[cold]
    async fn read_slow(&self) {
        loop {
            let l = self.readers.listen();

            if self.try_read() {
                return;
            }

            l.await;

            if self.try_read() {
                return;
            }
        }
    }

    pub fn try_write(&self) -> bool {
        self.state
            .compare_exchange(UNLOCKED, WRITER, Ordering::Acquire, Ordering::Relaxed)
            .is_ok()
    }

    pub async fn write(&self) {
        if self.try_write() {
            return;
        }

        self.write_slow().await;
    }

    #[cold]
    async fn write_slow(&self) {
        loop {
            let l = self.writers.listen();

            if self.try_write() {
                return;
            }

            l.await;

            if self.try_write() {
                return;
            }
        }
    }

    /// # Safety
    /// unlocking is only safe when the MutexGuard is lost
    /// and its drop never ran, like with mem::forget
    pub unsafe fn write_unlock(&self) {
        self.state.fetch_and(!(WRITER | UPGRADE), Ordering::Release);
        self.readers.notify(usize::MAX);
        self.writers.notify(1);
    }
}

impl Default for Lock {
    fn default() -> Self {
        Self::new()
    }
}

const UNLOCKED: usize = 0;
const WRITER: usize = 1 << 0;
const UPGRADE: usize = 1 << 1;
const READER: usize = 1 << 2;
const MAX_READERS: usize = (usize::MAX >> 3) << 2;
