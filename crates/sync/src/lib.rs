#![no_std]
#![feature(const_caller_location, custom_test_frameworks, test)]

//

pub mod spinlock;

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
