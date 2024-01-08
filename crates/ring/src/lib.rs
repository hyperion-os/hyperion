#![no_std]
#![feature(inline_const, new_uninit)]

//

extern crate alloc;

//

use alloc::boxed::Box;
use core::{
    cell::UnsafeCell,
    mem::{ManuallyDrop, MaybeUninit},
    sync::atomic::{AtomicUsize, Ordering},
};

use crossbeam::utils::CachePadded;

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
#[repr(C)]
pub struct RingBufMarker {
    read: CachePadded<AtomicUsize>,
    write: CachePadded<AtomicUsize>,
    // len: CachePadded<AtomicUsize>,
    capacity: usize,
}

impl RingBufMarker {
    #[must_use]
    pub const fn new(len: usize) -> Self {
        assert!(len != 0);

        Self {
            read: CachePadded::new(AtomicUsize::new(0)),
            write: CachePadded::new(AtomicUsize::new(0)),
            // len: CachePadded::new(AtomicUsize::new(0)),
            capacity: len,
        }
    }

    #[must_use]
    pub fn uninit_slot(&self) -> Slot {
        let write = self.write.load(Ordering::Acquire);
        let read = self.read.load(Ordering::Acquire);
        // let len = self.len.load(Ordering::Acquire);

        // read end - 1 is the limit, the number of available spaces can only grow
        // read=write would be ambiguous so read=write always means that the whole buf is empty
        // => write of self.len to an empty buffer is not possible (atm)
        let avail = if write < read {
            read - write
        } else {
            self.capacity - write + read
        };
        assert!(avail <= self.capacity);

        Slot::new(write, avail - 1)
        // Slot::new(
        //     write,
        //     self.capacity.checked_sub(len).unwrap_or_else(|| {
        //         panic!("uninit_slot_panic({write} {len} {})", self.capacity);
        //     }),
        // )
    }

    #[must_use]
    pub fn init_slot(&self) -> Slot {
        let read = self.read.load(Ordering::Acquire);
        let write = self.write.load(Ordering::Acquire);
        // let len = self.len.load(Ordering::Acquire);

        // write end is the limit, the number of available items can only grow
        let avail = if read <= write {
            write - read
        } else {
            self.capacity - read + write
        };
        assert!(avail <= self.capacity);

        Slot::new(read, avail)
        // Slot::new(read, len)
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
        let new_write_end = (acquire.first + acquire.len) % self.capacity;
        let old = self.write.swap(new_write_end, Ordering::Release);
        assert_eq!(old, acquire.first);
        // self.len.fetch_add(acquire.len, Ordering::Release);
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
        // self.len.fetch_sub(consume.len, Ordering::Release);
        let new_read_end = (consume.first + consume.len) % self.capacity;
        let old = self.read.swap(new_read_end, Ordering::Release);
        assert_eq!(old, consume.first);
    }
}

//

/* #[repr(C)]
pub struct Ring<T> {
    marker: RingBufMarker,
    items: Box<[UnsafeCell<MaybeUninit<T>>]>,
}

impl<T> Ring<T> {
    pub fn new(capacity: usize) -> Self {
        Self {
            marker: RingBufMarker::new(capacity),
            items: (0..capacity)
                .map(|_| UnsafeCell::new(MaybeUninit::uninit()))
                .collect(),
        }
    }

    pub fn push(&mut self, val: T) -> Result<(), T> {
        match self.push_iter(1, [val]) {
            Ok(()) => Ok(()),
            Err([val]) => Err(val),
        }
    }

    pub fn push_arr<const LEN: usize>(&mut self, val: [T; LEN]) -> Result<(), [T; LEN]> {
        match self.push_iter(val.len(), val) {
            Ok(()) => Ok(()),
            Err(val) => Err(val),
        }
    }

    pub fn pop(&mut self) -> Option<T> {
        // Self::release(&mut self.marker, 1, |slot| {
        //     let (beg, end) = slot.slices(&self.items);
        //     debug_assert!(beg.len() == 1 && end.is_empty());

        //     let slot = unsafe { beg[0].get().as_mut() }.unwrap();
        //     unsafe { slot.assume_init_read() }
        // })
        todo!()
    }

    pub fn pop_slice(&mut self, _buf: &mut [T]) -> Option<()> {
        // if let Some(slot) = unsafe { marker.acquire(buf.len()) } {
        //     // f should write the items
        //     f(&slot, val);

        //     // mark it as readable
        //     unsafe { marker.produce(slot) };
        //     Ok(())
        // } else {
        //     Err(val)
        // }
        todo!()
    }

    pub fn push_iter<I>(&mut self, count: usize, iter: I) -> Result<(), I>
    where
        I: IntoIterator<Item = T>,
        I::IntoIter: ExactSizeIterator,
    {
        if let Some(slot) = unsafe { self.marker.consume(count) } {
            let (beg, end) = slot.slices(&self.items);
            debug_assert_eq!(beg.len() + end.len(), count);

            let iter = iter.into_iter();
            debug_assert_eq!(count, iter.len());
            for (to, from) in beg.iter().chain(end).zip(iter) {
                unsafe { to.get().as_mut() }.unwrap().write(from);
            }

            // mark it as writeable
            unsafe { self.marker.release(slot) };
            Ok(())
        } else {
            Err(iter)
        }
    }

    fn release(&mut self, count: usize) -> Result<ReleaseSlot<UnsafeCell<MaybeUninit<T>>>, ()> {
        if let Some(slot) = unsafe { self.marker.consume(count) } {
            Ok(ReleaseSlot {
                marker: &self.marker,
                items: &self.items,
                consume: ManuallyDrop::new(slot),
            })
        } else {
            Err(())
        }
    }
}

impl<T: Copy> Ring<T> {
    pub fn push_slice(&mut self, val: &[T]) -> Option<()> {
        let s = self.release(val.len()).ok()?;

        s.slices();

        self.push_iter(val.len(), val.iter().copied()).ok()
    }
}

//

struct ReleaseSlot<'a, T> {
    marker: &'a RingBufMarker,
    items: &'a [UnsafeCell<MaybeUninit<T>>],
    consume: ManuallyDrop<Slot>,
}

impl<'a, T> ReleaseSlot<'a, T> {
    // pub fn slices(&self) -> (&'a [T], &'a [T]) {
    //     self.consume.slices(self.items)
    // }

    pub fn iter(&self) -> impl ExactSizeIterator<Item = &mut MaybeUninit<T>> {
        let (beg, end) = self.consume.slices(self.items);
        beg.iter()
            .chain(end)
            .map(|cell| unsafe { cell.get().as_mut() })
    }
}

impl<'a, T> Drop for ReleaseSlot<'a, T> {
    fn drop(&mut self) {
        // mark it as writeable
        let slot = unsafe { ManuallyDrop::take(&mut self.consume) };
        unsafe { self.marker.release(slot) };
    }
}

struct SliceTuple<'a, T>(&'a [T], &'a [T]);

impl<'a, T> Iterator for SliceTuple<'a, T> {
    type Item = &'a T;

    fn next(&mut self) -> Option<Self::Item> {
        self.0.split_first()?;
    }
}

impl<'a, T> ExactSizeIterator for SliceTuple<'a, T> {
    fn len(&self) -> usize {
        let (lower, upper) = self.size_hint();
        // Note: This assertion is overly defensive, but it checks the invariant
        // guaranteed by the trait. If this trait were rust-internal,
        // we could use debug_assert!; assert_eq! will check all Rust user
        // implementations too.
        assert_eq!(upper, Some(lower));
        lower
    }

    fn is_empty(&self) -> bool {
        self.len() == 0
    }
} */

//

#[cfg(test)]
mod tests {
    extern crate std;

    use crate::RingBufMarker;

    //

    #[test]
    fn init_empty() {
        let marker = RingBufMarker::new(5);

        assert_eq!(marker.free_space(), 4);
        assert_eq!(marker.used_space(), 0);

        assert_eq!(
            unsafe { marker.consume(3) },
            None,
            "the ring buf should be empty"
        );
    }

    #[test]
    fn fill() {
        let marker = RingBufMarker::new(5);

        let slot = unsafe { marker.acquire(4) }.unwrap();
        unsafe { marker.produce(slot) };
        assert_eq!(marker.free_space(), 0);
        assert_eq!(marker.used_space(), 4);
    }

    #[test]
    fn fill_offset() {
        let marker = RingBufMarker::new(5);

        let slot = unsafe { marker.acquire(2) }.unwrap();
        unsafe { marker.produce(slot) };
        assert_eq!(marker.free_space(), 2);
        assert_eq!(marker.used_space(), 2);

        let slot = unsafe { marker.consume(1) }.unwrap();
        unsafe { marker.release(slot) };
        assert_eq!(marker.free_space(), 3);
        assert_eq!(marker.used_space(), 1);

        let slot = unsafe { marker.consume(1) }.unwrap();
        unsafe { marker.release(slot) };
        assert_eq!(marker.free_space(), 4);
        assert_eq!(marker.used_space(), 0);

        let slot = unsafe { marker.acquire(4) }.unwrap();
        unsafe { marker.produce(slot) };
        assert_eq!(marker.free_space(), 0);
        assert_eq!(marker.used_space(), 4);
    }

    #[test]
    fn rw() {
        let marker = RingBufMarker::new(255);

        let slot = unsafe { marker.acquire(63) }.unwrap();
        unsafe { marker.produce(slot) };
        assert_eq!(marker.free_space(), 191);
        assert_eq!(marker.used_space(), 63);

        let slot = unsafe { marker.consume(140) };
        assert_eq!(slot, None);
    }
}
