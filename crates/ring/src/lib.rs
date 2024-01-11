#![no_std]
#![feature(
    inline_const,
    new_uninit,
    maybe_uninit_uninit_array,
    maybe_uninit_array_assume_init
)]

//

extern crate alloc;

//

use alloc::boxed::Box;
use core::{
    cell::UnsafeCell,
    marker::PhantomData,
    mem::{ManuallyDrop, MaybeUninit},
    ops::Deref,
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
    pub fn min(self, n: usize) -> Self {
        Self::new(self.first(), self.len().min(n))
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
    /// this is a **write** operation, see [`Self`]
    pub unsafe fn acquire(&self, count: usize) -> Option<Slot> {
        if self.capacity < count {
            return None;
        }

        self.uninit_slot().take(count)
    }

    /// # Safety
    /// this is a **write** operation, see [`Self`]
    pub unsafe fn acquire_up_to(&self, count: usize) -> Slot {
        self.uninit_slot().min(count)
    }

    /// # Safety
    /// this is a **write** operation, see [`Self`]
    pub unsafe fn produce(&self, acquire: Slot) {
        let new_write_end = (acquire.first + acquire.len) % self.capacity;
        let old = self.write.swap(new_write_end, Ordering::Release);
        assert_eq!(old, acquire.first);
        // self.len.fetch_add(acquire.len, Ordering::Release);
    }

    /// # Safety
    /// this is a **read** operation, see [`Self`]
    pub unsafe fn consume(&self, count: usize) -> Option<Slot> {
        if self.capacity < count {
            return None;
        }

        self.init_slot().take(count)
    }

    /// # Safety
    /// this is a **read** operation, see [`Self`]
    pub unsafe fn consume_up_to(&self, count: usize) -> Slot {
        self.init_slot().min(count)
    }

    /// # Safety
    /// this is a **read** operation, see [`Self`]
    pub unsafe fn release(&self, consume: Slot) {
        // self.len.fetch_sub(consume.len, Ordering::Release);
        let new_read_end = (consume.first + consume.len) % self.capacity;
        let old = self.read.swap(new_read_end, Ordering::Release);
        assert_eq!(old, consume.first);
    }

    /// # Safety
    /// this is both **write** operations combined into one, see [`Self`]
    pub unsafe fn write_guard(&self, count: usize) -> Option<impl Deref<Target = Slot> + '_> {
        unsafe { self.acquire(count) }.map(|slot| WriteGuard::new(slot, self))
    }

    /// # Safety
    /// this is both **write** operations combined into one, see [`Self`]
    pub unsafe fn write_guard_up_to(&self, count: usize) -> impl Deref<Target = Slot> + '_ {
        let slot = unsafe { self.acquire_up_to(count) };
        WriteGuard::new(slot, self)
    }

    /// # Safety
    /// this is both **read** operations combined into one, see [`Self`]
    pub unsafe fn read_guard(&self, count: usize) -> Option<impl Deref<Target = Slot> + '_> {
        unsafe { self.consume(count) }.map(|slot| ReadGuard::new(slot, self))
    }

    /// # Safety
    /// this is both **read** operations combined into one, see [`Self`]
    pub unsafe fn read_guard_up_to(&self, count: usize) -> impl Deref<Target = Slot> + '_ {
        let slot = unsafe { self.consume_up_to(count) };
        ReadGuard::new(slot, self)
    }
}

//

struct WriteGuard<'a> {
    slot: ManuallyDrop<Slot>,
    marker: &'a RingBufMarker,
}

impl<'a> WriteGuard<'a> {
    fn new(slot: Slot, marker: &'a RingBufMarker) -> Self {
        Self {
            slot: ManuallyDrop::new(slot),
            marker,
        }
    }
}

impl Deref for WriteGuard<'_> {
    type Target = Slot;

    fn deref(&self) -> &Self::Target {
        &self.slot
    }
}

impl Drop for WriteGuard<'_> {
    fn drop(&mut self) {
        // mark it as readable
        unsafe { self.marker.produce(ManuallyDrop::take(&mut self.slot)) };
    }
}

//

struct ReadGuard<'a> {
    slot: ManuallyDrop<Slot>,
    marker: &'a RingBufMarker,
}

impl<'a> ReadGuard<'a> {
    fn new(slot: Slot, marker: &'a RingBufMarker) -> Self {
        Self {
            slot: ManuallyDrop::new(slot),
            marker,
        }
    }
}

impl Deref for ReadGuard<'_> {
    type Target = Slot;

    fn deref(&self) -> &Self::Target {
        &self.slot
    }
}

impl Drop for ReadGuard<'_> {
    fn drop(&mut self) {
        // mark it as writeable
        unsafe { self.marker.release(ManuallyDrop::take(&mut self.slot)) };
    }
}

//

pub trait Storage<T>: Deref<Target = [UnsafeCell<MaybeUninit<T>>]> {}

impl<T> Storage<T> for T where T: Deref<Target = [UnsafeCell<MaybeUninit<T>>]> {}

//

// pub struct Sender<T, C> {
//     inner: Arc<RingBuf<T, C>>,
// }

// impl<T, C> Sender<T, C> where C: Storage<T> {}

//

// pub struct Receiver<T, C> {
//     inner: Arc<RingBuf<T, C>>,
// }

//

pub type StaticRingBuf<T, const N: usize> = RingBuf<T, Static<T, N>>;

// pub type RefRingBuf<'a, T> = RingBuf<T, &'a [UnsafeCell<MaybeUninit<T>>]>;

// pub type OwnedRingBuf<T> = RingBuf<T, Box<[UnsafeCell<MaybeUninit<T>>]>>;

// pub struct StaticRingBuf<T, const N: usize> {
//     inner: RingBuf<T, [UnsafeCell<MaybeUninit<T>>; N]>,
//     read:
// }

pub struct Static<T, const N: usize>([UnsafeCell<MaybeUninit<T>>; N]);

impl<T, const N: usize> Static<T, N> {
    pub const fn new() -> Self {
        // this `const {}` shit is just amazing
        Self([const { UnsafeCell::new(MaybeUninit::uninit()) }; N])
    }
}

impl<T, const N: usize> Deref for Static<T, N> {
    type Target = [UnsafeCell<MaybeUninit<T>>];

    fn deref(&self) -> &Self::Target {
        &self.0[..]
    }
}

//

#[macro_export]
macro_rules! static_ringbuf {
    ($t:ty, $len:expr) => {{
        static RINGBUF: $crate::RingBuf<$t, $crate::Static<$t, $len>> =
            $crate::RingBuf::<$t, $crate::Static<$t, $len>>::new();
        let tx = unsafe { $crate::RefSender::from_inner(&RINGBUF) };
        let rx = unsafe { $crate::RefReceiver::from_inner(&RINGBUF) };
        (tx, rx)
    }};
}

//

pub struct RefSender<'a, T: 'a, C> {
    inner: &'a RingBuf<T, C>,
}

impl<'a, T, C> RefSender<'a, T, C> {
    /// # Safety
    /// not safe
    pub const unsafe fn from_inner(inner: &'a RingBuf<T, C>) -> Self {
        Self { inner }
    }
}

impl<'a, T, C> RefSender<'a, T, C>
where
    T: 'a,
    C: Storage<T>,
{
    pub fn push(&mut self, val: T) -> Result<(), T> {
        unsafe { self.inner.push(val) }
    }

    pub fn push_arr<const LEN: usize>(&self, val: [T; LEN]) -> Result<(), [T; LEN]> {
        unsafe { self.inner.push_arr(val) }
    }
}

impl<'a, T, C> RefSender<'a, T, C>
where
    T: Copy + 'a,
    C: Storage<T>,
{
    pub fn push_slice(&mut self, buf: &[T]) -> usize {
        unsafe { self.inner.push_slice(buf) }
    }
}

//

pub struct RefReceiver<'a, T: 'a, C> {
    inner: &'a RingBuf<T, C>,
}

impl<'a, T, C> RefReceiver<'a, T, C>
where
    T: 'a,
    C: Storage<T>,
{
    pub fn pop(&self) -> Option<T> {
        unsafe { self.inner.pop() }
    }

    pub fn pop_arr<const LEN: usize>(&self) -> Option<[T; LEN]> {
        unsafe { self.inner.pop_arr() }
    }
}

impl<'a, T, C> RefReceiver<'a, T, C>
where
    T: Copy + 'a,
    C: Storage<T>,
{
    pub fn pop_slice(&mut self, buf: &mut [T]) -> usize {
        unsafe { self.inner.pop_slice(buf) }
    }
}

//

#[repr(C)]
pub struct RingBuf<T, C> {
    marker: RingBufMarker,
    items: C,
    _p: PhantomData<T>,
}

unsafe impl<T: Send, C> Sync for RingBuf<T, C> {}

impl<T, C> RingBuf<T, C> {
    pub const fn from(items: C, capacity: usize) -> Self {
        Self {
            marker: RingBufMarker::new(capacity),
            items,
            _p: PhantomData,
        }
    }
}

impl<T, const N: usize> RingBuf<T, Static<T, N>> {
    pub const fn new() -> Self {
        Self::from(Static::new(), N)
    }
}

impl<T> RingBuf<T, Box<[UnsafeCell<MaybeUninit<T>>]>> {
    pub fn new(capacity: usize) -> Self {
        Self::from(
            (0..capacity)
                .map(|_| UnsafeCell::new(MaybeUninit::uninit()))
                .collect(),
            capacity,
        )
    }
}

impl<T, C> RingBuf<T, C>
where
    C: Storage<T>,
{
    /// # Safety
    /// this is a **write** operation, see [`Self`]
    pub unsafe fn push(&self, val: T) -> Result<(), T> {
        if let Some(slot) = unsafe { self.marker.write_guard(1) } {
            unsafe { self.write(&slot, [val]) };
            Ok(())
        } else {
            Err(val)
        }
    }

    /// # Safety
    /// this is a **write** operation, see [`Self`]
    pub unsafe fn push_arr<const LEN: usize>(&self, val: [T; LEN]) -> Result<(), [T; LEN]> {
        if let Some(slot) = unsafe { self.marker.write_guard(1) } {
            unsafe { self.write(&slot, val) };
            Ok(())
        } else {
            Err(val)
        }
    }

    /// # Safety
    /// this is a **read** operation, see [`Self`]
    pub unsafe fn pop(&self) -> Option<T> {
        if let Some(slot) = unsafe { self.marker.read_guard(1) } {
            let item = unsafe { self.read(&slot) }.next().unwrap();
            Some(item)
        } else {
            None
        }
    }

    /// # Safety
    /// this is a **read** operation, see [`Self`]
    pub unsafe fn pop_arr<const LEN: usize>(&self) -> Option<[T; LEN]> {
        if let Some(slot) = unsafe { self.marker.read_guard(LEN) } {
            let mut buf = MaybeUninit::uninit_array();
            for (from, to) in unsafe { self.read(&slot) }.zip(buf.iter_mut()) {
                to.write(from);
            }
            Some(unsafe { MaybeUninit::array_assume_init(buf) })
        } else {
            None
        }
    }

    unsafe fn write<I>(&self, slot: &Slot, iter: I)
    where
        I: IntoIterator<Item = T>,
        I::IntoIter: ExactSizeIterator,
    {
        let iter = iter.into_iter();
        let (beg, end) = slot.slices(&self.items);
        debug_assert_eq!(beg.len() + end.len(), iter.len());

        for (to, from) in beg.iter().chain(end).zip(iter) {
            unsafe { to.get().as_mut() }.unwrap().write(from);
        }
    }

    unsafe fn read(&self, slot: &Slot) -> impl ExactSizeIterator<Item = T> + '_ {
        let (beg, end) = slot.slices(&self.items);

        ExactSizeChain(beg.iter(), end.iter())
            .map(|cell| unsafe { (*cell.get()).assume_init_read() })
    }
}

impl<T, C> RingBuf<T, C>
where
    T: Copy,
    C: Storage<T>,
{
    /// # Safety
    /// this is a **write** operation, see [`Self`]
    pub unsafe fn push_slice(&self, buf: &[T]) -> usize {
        let slot = unsafe { self.marker.write_guard_up_to(buf.len()) };

        unsafe { self.write(&slot, buf.iter().copied()) };

        slot.len
    }

    /// # Safety
    /// this is a **read** operation, see [`Self`]
    pub unsafe fn pop_slice(&self, buf: &mut [T]) -> usize {
        let slot = unsafe { self.marker.read_guard_up_to(buf.len()) };

        for (from, to) in unsafe { self.read(&slot) }.zip(buf) {
            *to = from;
        }

        slot.len
    }
}

//

struct ExactSizeChain<A, B>(A, B);

impl<A, B> Iterator for ExactSizeChain<A, B>
where
    A: Iterator,
    B: Iterator<Item = A::Item>,
{
    type Item = A::Item;

    fn next(&mut self) -> Option<Self::Item> {
        self.0.next().or_else(|| self.1.next())
    }
}

impl<A, B> ExactSizeIterator for ExactSizeChain<A, B>
where
    A: ExactSizeIterator,
    B: ExactSizeIterator<Item = A::Item>,
{
    fn len(&self) -> usize {
        self.0.len() + self.1.len()
    }
}

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
