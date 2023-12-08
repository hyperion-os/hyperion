#![no_std]

//

use alloc::boxed::Box;
use core::{cell::UnsafeCell, ops::Deref};

pub use x86_64::*;

//

extern crate alloc;

mod x86_64;

//

pub struct Tls<T: 'static> {
    inner: Box<[UnsafeCell<T>]>,
}

unsafe impl<T: Sync> Sync for Tls<T> {}

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
