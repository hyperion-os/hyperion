#![no_std]
#![feature(const_caller_location, custom_test_frameworks, test)]

//

// use core::{ptr::NonNull, sync::atomic::AtomicUsize};

// use crossbeam::atomic::AtomicCell;

// pub mod mutex;
pub mod spinlock;

//

// pub fn init_futex(wait: fn(NonNull<AtomicUsize>, usize), wake: fn(NonNull<AtomicUsize>, usize)) {
//     FUTEX_WAIT.store(wait);
//     FUTEX_WAKE.store(wake);
// }

//

// TODO: this is horrible
// static FUTEX_WAIT: AtomicCell<fn(NonNull<AtomicUsize>, usize)> =
//     AtomicCell::new(|_, _| panic!("futex not initialized"));

// static FUTEX_WAKE: AtomicCell<fn(NonNull<AtomicUsize>, usize)> =
//     AtomicCell::new(|_, _| panic!("futex not initialized"));

//

#[macro_export]
macro_rules! once {
    () => {{
        use core::sync::atomic::{AtomicBool, Ordering};
        static ONCE: AtomicBool = AtomicBool::new(true);
        ONCE.swap(false, Ordering::SeqCst)
    }};
}

//

#[cfg(test)]
mod tests {}
