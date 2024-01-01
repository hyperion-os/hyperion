#![no_std]
#![feature(inline_const)]

//

extern crate alloc;

use core::cmp;

use crossbeam::utils::CachePadded;
use sync::*;

//

pub(crate) mod sync {
    #[cfg(not(all(loom, not(target_os = "none"))))]
    pub use core::sync::atomic::{AtomicUsize, Ordering};

    #[cfg(all(loom, not(target_os = "none")))]
    pub use loom::sync::atomic::{AtomicUsize, Ordering};
}

//

#[derive(Debug, PartialEq, Eq)]
pub struct Slot {
    first: usize,
    len: usize,
}

impl Slot {
    #[must_use]
    pub const fn new(first: usize, len: usize) -> Self {
        Self { first, len }
    }

    #[must_use]
    pub const fn take(self, n: usize) -> Option<Self> {
        if self.len < n {
            None
        } else {
            Some(Self::new(self.first(), n))
        }
    }

    #[must_use]
    pub const fn first(&self) -> usize {
        self.first
    }

    #[must_use]
    pub const fn len(&self) -> usize {
        self.len
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn slices<'a, T>(&self, slice: &'a [T]) -> (&'a [T], &'a [T]) {
        assert!(self.len() <= slice.len());

        if self.first() + self.len() <= slice.len() {
            (&slice[self.first()..self.first() + self.len()], &[])
        } else {
            let first = &slice[self.first()..];
            (first, &slice[..self.len() - first.len()])
        }
    }
}

//

/// # Safety
/// Write ops are not in sync with other write ops,
/// read ops are not in sync with other read ops,
/// write ops are in sync with read ops.
///
/// [`RingBufMarker::acquire`] should be paired with [`RingBufMarker::produce`]
/// and [`RingBufMarker::consume`] should be paired with [`RingBufMarker::release`]
///
/// [`RingBufMarker::acquire`] after [`RingBufMarker::acquire`]
/// invalidates the first acquired slot and likewise
/// [`RingBufMarker::consume`] after [`RingBufMarker::consume`]
/// invalidates the first consumed slot
#[derive(Debug)]
pub struct RingBufMarker {
    read: CachePadded<AtomicUsize>,
    write: CachePadded<AtomicUsize>,
    len: CachePadded<AtomicUsize>,
    capacity: usize,
}

impl RingBufMarker {
    #[cfg(not(all(loom, not(target_os = "none"))))]
    #[must_use]
    pub const fn new(len: usize) -> Self {
        Self {
            read: CachePadded::new(AtomicUsize::new(0)),
            write: CachePadded::new(AtomicUsize::new(0)),
            len: CachePadded::new(AtomicUsize::new(0)),
            capacity: len,
        }
    }

    #[cfg(all(loom, not(target_os = "none")))]
    #[must_use]
    pub fn new(len: usize) -> Self {
        Self {
            read: CachePadded::new(AtomicUsize::new(0)),
            write: CachePadded::new(AtomicUsize::new(0)),
            len,
        }
    }

    #[must_use]
    pub fn uninit_slot(&self) -> Slot {
        let write = self.write.load(Ordering::Acquire);
        let read = self.read.load(Ordering::Acquire);

        // read end - 1 is the limit, the number of available spaces can only grow
        // read=write would be ambiguous so read=write always means that the whole buf is empty
        // => write of self.len to an empty buffer is not possible (atm)
        let avail = if write < read {
            read - write
        } else {
            self.capacity - read + write
        };

        Slot::new(write, avail - 1)
    }

    #[must_use]
    pub fn init_slot(&self) -> Slot {
        let read = self.read.load(Ordering::Acquire);
        let write = self.write.load(Ordering::Acquire);

        // write end is the limit, the number of available items can only grow
        let avail = if read <= write {
            write - read
        } else {
            self.capacity - write + read
        };

        Slot::new(read, avail)
    }

    #[must_use]
    pub fn free_space(&self) -> usize {
        self.uninit_slot().len()
    }

    #[must_use]
    pub fn used_space(&self) -> usize {
        self.init_slot().len()
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.used_space()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// # Safety
    /// this is a write operation, see [`Self`]
    pub unsafe fn acquire(&self, count: usize) -> Option<Slot> {
        if self.capacity < count {
            return None;
        }

        self.uninit_slot().take(count)
    }

    /// # Safety
    /// this is a write operation, see [`Self`]
    pub unsafe fn produce(&self, acquire: Slot) {
        self.write.store(
            (acquire.first + acquire.len) % self.capacity,
            Ordering::Release,
        );
        self.len.fetch_add(acquire.len, Ordering::Release);
    }

    /// # Safety
    /// this is a read operation, see [`Self`]
    pub unsafe fn consume(&self, count: usize) -> Option<Slot> {
        if self.capacity < count {
            return None;
        }

        self.init_slot().take(count)
    }

    /// # Safety
    /// this is a read operation, see [`Self`]
    pub unsafe fn release(&self, consume: Slot) {
        self.len.fetch_sub(consume.len, Ordering::Release);
        self.read.store(
            (consume.first + consume.len) % self.capacity,
            Ordering::Release,
        );
    }
}

#[cfg(test)]
mod tests {
    extern crate std;

    use std::dbg;

    use sync::*;

    use crate::RingBufMarker;

    //

    pub(crate) mod sync {
        extern crate std;

        #[cfg(not(all(loom, not(target_os = "none"))))]
        pub use std::{
            sync::{Arc, Barrier, Mutex},
            thread,
        };

        #[cfg(all(loom, not(target_os = "none")))]
        pub use loom::{
            sync::{Arc, Barrier, Mutex},
            thread,
        };
    }

    //

    macro_rules! clone_move {
        ([$($i:ident),*] $($closure:tt)*) => {{
            $(let $i = $i.clone();)*

            move || {
                $($closure)*
            }
        }};
    }

    //

    #[test]
    fn init_empty() {
        let marker = RingBufMarker::new(4);
        assert_eq!(
            unsafe { marker.consume(3) },
            None,
            "the ring buf should be empty"
        );
    }

    #[test]
    fn fill() {
        let marker = RingBufMarker::new(4);
        unsafe { marker.acquire(4) }.expect("the ring buf should be empty");
    }

    #[test]
    fn fill_offset() {
        let marker = RingBufMarker::new(4);

        assert_eq!(marker.free_space(), 3);
        assert_eq!(marker.used_space(), 0);

        let slot = unsafe { marker.acquire(2) }.expect("the ring buf should be empty");
        unsafe { marker.produce(slot) };

        assert_eq!(marker.free_space(), 1);
        assert_eq!(marker.used_space(), 0);

        assert!(
            unsafe { marker.consume(1) }.is_none(),
            "the ring buf should be empty"
        );

        dbg!(&marker);
        let slot = unsafe { marker.consume(2) }.expect("the ring buf should have 2 items");
        dbg!(&marker);
        unsafe { marker.release(slot) };

        dbg!(&marker);
        unsafe { marker.acquire(4) }.expect("the ring buf should be empty");

        dbg!(&marker);
        assert_eq!(
            unsafe { marker.acquire(1) },
            None,
            "the ring buf should be full"
        );
    }

    #[test]
    fn loom_test() {
        let run = || {
            let marker = Arc::new(RingBufMarker::new(4));
            let arr = Arc::new([const { Mutex::new(()) }; 4]);

            let barrier_0 = Arc::new(Barrier::new(1));
            let barrier_1 = Arc::new(Barrier::new(1));

            assert_eq!(
                unsafe { marker.consume(3) },
                None,
                "the ring buf should be empty"
            );

            let t0 = thread::spawn(clone_move! { [marker, arr, barrier_0]
                let slot = unsafe { marker.acquire(3) }.unwrap();
                let (a, b) = slot.slices(&arr[..]);
                std::println!("{a:?} {b:?}");
                for item in a.iter().chain(b) {
                    drop(item.try_lock().unwrap());
                }
                unsafe { marker.produce(slot) };

                barrier_0.wait();
            });

            let t1 = thread::spawn(clone_move! { [marker, arr, barrier_1]
                let slot = unsafe { marker.acquire(3) }.unwrap();
                let (a, b) = slot.slices(&arr[..]);
                std::println!("{a:?} {b:?}");
                for item in a.iter().chain(b) {
                    drop(item.try_lock().unwrap());
                }
                unsafe { marker.produce(slot) };

                barrier_1.wait();
            });

            let t2 = thread::spawn(clone_move! { [marker, arr, barrier_0]
                barrier_0.wait();

                let slot = unsafe { marker.consume(3) }.unwrap();
                let (a, b) = slot.slices(&arr[..]);
                std::println!("{a:?} {b:?}");
                for item in a.iter().chain(b) {
                    drop(item.try_lock().unwrap());
                }
                unsafe { marker.release(slot) };
            });

            let t3 = thread::spawn(clone_move! { [marker, arr, barrier_1]
                barrier_1.wait();

                let slot = unsafe { marker.consume(3) }.unwrap();
                let (a, b) = slot.slices(&arr[..]);
                std::println!("{a:?} {b:?}");
                for item in a.iter().chain(b) {
                    drop(item.try_lock().unwrap());
                }
                unsafe { marker.release(slot) };
            });

            t0.join().unwrap();
            t1.join().unwrap();
            t2.join().unwrap();
            t3.join().unwrap();
        };

        #[cfg(not(all(loom, not(target_os = "none"))))]
        run();
        #[cfg(all(loom, not(target_os = "none")))]
        loom::model(run);

        panic!();
    }
}
