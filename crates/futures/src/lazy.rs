use core::{
    cell::{Cell, UnsafeCell},
    future::Future,
    mem::MaybeUninit,
    ptr,
    sync::atomic::{AtomicU8, Ordering},
};

use event_listener::Event;

//

pub struct Lazy<T, F> {
    once: Once<T>,
    f: Cell<Option<F>>,
}

unsafe impl<T, F: Send> Sync for Lazy<T, F> where Once<T>: Sync {}

impl<T, F: Future<Output = T>> Lazy<T, F> {
    pub const fn new(f: F) -> Self {
        Self {
            once: Once::new(),
            f: Cell::new(Some(f)),
        }
    }

    pub async fn get(&self) -> &T {
        self.once
            .call_once(async { self.f.take().unwrap().await })
            .await
    }
}

//

pub struct Once<T> {
    val: UnsafeCell<MaybeUninit<T>>,
    complete: AtomicU8,
    waiting: Event,
}

unsafe impl<T: Send + Sync> Sync for Once<T> {}
unsafe impl<T: Send> Send for Once<T> {}

impl<T> Once<T> {
    pub const fn new() -> Self {
        Self {
            val: UnsafeCell::new(MaybeUninit::uninit()),
            complete: AtomicU8::new(INCOMPLETE),
            waiting: Event::new(),
        }
    }

    pub async fn call_once(&self, f: impl Future<Output = T>) -> &T {
        if let Some(val) = self.get() {
            val
        } else {
            self.call_once_slow(f).await
        }
    }

    pub fn get(&self) -> Option<&T> {
        if self.is_complete() {
            Some(unsafe { self.get_force() })
        } else {
            None
        }
    }

    pub async fn poll(&self) -> Option<&T> {
        loop {
            match self.complete.load(Ordering::Acquire) {
                INCOMPLETE => return None,
                RUNNING => {
                    self.waiting.listen().await;
                    continue;
                }
                COMPLETE => return Some(unsafe { self.get_force() }),
                _ => unreachable!(),
            }
        }
    }

    pub async fn wait(&self) -> &T {
        loop {
            match self.complete.load(Ordering::Acquire) {
                INCOMPLETE | RUNNING => {
                    self.waiting.listen().await;
                    continue;
                }
                COMPLETE => return unsafe { self.get_force() },
                _ => unreachable!(),
            }
        }
    }

    pub fn is_complete(&self) -> bool {
        self.complete.load(Ordering::Acquire) == COMPLETE
    }

    #[cold]
    async fn call_once_slow(&self, f: impl Future<Output = T>) -> &T {
        loop {
            let result = self.complete.compare_exchange(
                INCOMPLETE,
                RUNNING,
                Ordering::Acquire,
                Ordering::Acquire,
            );

            match result {
                Ok(_incomplete) => {}
                Err(RUNNING) => {
                    if let Some(val) = self.poll().await {
                        return val;
                    } else {
                        continue;
                    }
                }
                Err(COMPLETE) => return unsafe { self.get_force() },
                _ => unreachable!(),
            };

            let new_val = MaybeUninit::new(f.await);
            unsafe { self.val.get().write(new_val) };
            self.complete.store(COMPLETE, Ordering::Release);

            return unsafe { self.get_force() };
        }
    }

    unsafe fn get_force(&self) -> &T {
        unsafe { (*self.val.get()).assume_init_ref() }
    }
}

impl<T> Default for Once<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> Drop for Once<T> {
    fn drop(&mut self) {
        if !self.is_complete() {
            return;
        }

        unsafe {
            ptr::drop_in_place((*self.val.get()).as_mut_ptr());
        }
    }
}

const INCOMPLETE: u8 = 0;
const RUNNING: u8 = 1;
const COMPLETE: u8 = 2;
