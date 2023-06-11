#![no_std]

//

pub struct Defer<F: FnOnce()> {
    f: Option<F>,
}

//

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
