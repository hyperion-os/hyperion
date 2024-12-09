#![no_std]

//

use alloc::boxed::Box;
use core::{cell::UnsafeCell, ops::Deref};

pub use x86_64::*;

//

extern crate alloc;

mod x86_64;

//

/// a bare metal thread local storage implementation based on cpu_id numbers and an array
pub struct Tls<T: 'static> {
    inner: Box<[UnsafeCell<T>]>,
}

unsafe impl<T> Sync for Tls<T> {}

impl<T: 'static + Default> Default for Tls<T> {
    fn default() -> Self {
        Self::new(T::default)
    }
}

impl<T: 'static> Tls<T> {
    pub fn new(mut f: impl FnMut() -> T) -> Self {
        Self {
            inner: (0..cpu_count()).map(|_| UnsafeCell::new(f())).collect(),
        }
    }

    pub fn inner(this: &Self) -> &[UnsafeCell<T>] {
        &this.inner
    }
}

impl<T: 'static> Deref for Tls<T> {
    type Target = T;

    #[inline(always)]
    fn deref(&self) -> &Self::Target {
        let tls_entry = self.inner[cpu_id()].get();

        // SAFETY: `cpu_id` is different for each cpu
        // TODO: not before cpu id is initialized
        unsafe { &*tls_entry }
    }
}

//

/* /// a Sync Cell that allows access only to the cpu that first accessed it
pub struct CpuIdCell<T: ?Sized> {
    owner: AtomicUsize,
    inner: UnsafeCell<T>,
}

impl<T: ?Sized> CpuIdCell<T> {
    pub const fn new(val: T) -> Self {
        Self {
            owner: AtomicUsize::new(usize::MAX),
            inner: UnsafeCell::new(val),
        }
    }
}

impl<T: ?Sized> Deref for CpuIdCell<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        let id = cpu_id();
        if id == usize::MAX {
            panic!();
        }

        self.owner
            .compare_exchange(usize::MAX, id, Ordering::SeqCst, Ordering::SeqCst)
            .is_err();

        todo!()
    }
} */
