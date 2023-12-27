#![no_std]

//

use core::{
    cell::{Cell, OnceCell},
    ops::{Deref, DerefMut},
};

pub struct Defer<F: FnOnce()> {
    f: Option<F>,
}

impl<F: FnOnce()> Defer<F> {
    pub const fn new(f: F) -> Self {
        Self { f: Some(f) }
    }

    pub fn take(mut self) -> F {
        self.f.take().unwrap()
    }
}

impl<F: FnOnce()> Drop for Defer<F> {
    fn drop(&mut self) {
        if let Some(f) = self.f.take() {
            f();
        }
    }
}

//

/// aka mutable unsync lazy
pub struct DeferInit<F: FnOnce() -> T, T> {
    f: Cell<Option<F>>,
    v: OnceCell<T>,
}

impl<F: FnOnce() -> T, T> DeferInit<F, T> {
    pub const fn new(f: F) -> Self {
        Self {
            f: Cell::new(Some(f)),
            v: OnceCell::new(),
        }
    }
}

impl<F: FnOnce() -> T, T> Deref for DeferInit<F, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.v.get_or_init(|| self.f.take().unwrap()())
    }
}

impl<F: FnOnce() -> T, T> DerefMut for DeferInit<F, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.v.get_or_init(|| self.f.take().unwrap()());
        self.v.get_mut().unwrap()
    }
}
