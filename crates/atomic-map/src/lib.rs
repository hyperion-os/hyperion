#![no_std]

//

extern crate alloc;

use alloc::boxed::Box;
use core::ptr;
#[cfg(not(loom))]
use core::sync::atomic::{AtomicPtr, AtomicUsize, Ordering};

#[cfg(loom)]
use loom::sync::atomic::{AtomicPtr, AtomicUsize, Ordering};

//

// actually just a linked list
pub struct AtomicMap<K, V> {
    len: AtomicUsize,
    head: AtomicPtr<AtomicMapNode<K, V>>,
}

struct AtomicMapNode<K, V> {
    key: K,
    value: V,
    next: AtomicPtr<AtomicMapNode<K, V>>,
}

//

impl<K, V> AtomicMap<K, V> {
    #[cfg(not(loom))]
    pub const fn new() -> Self {
        Self {
            len: AtomicUsize::new(0),
            head: AtomicPtr::new(ptr::null_mut()),
        }
    }

    #[cfg(loom)]
    pub fn new() -> Self {
        Self {
            len: AtomicUsize::new(0),
            head: AtomicPtr::new(ptr::null_mut()),
        }
    }

    pub fn insert(&self, key: K, value: V) {
        // TODO: stop if the key is already there
        let new = Box::into_raw(Box::new(AtomicMapNode {
            key,
            value,
            next: AtomicPtr::new(ptr::null_mut()),
        }));
        let mut original = self.head.load(Ordering::Acquire);

        loop {
            unsafe {
                (*new).next = AtomicPtr::new(original);
            }

            if let Err(head_changed) =
                self.head
                    .compare_exchange_weak(original, new, Ordering::AcqRel, Ordering::Acquire)
            {
                original = head_changed;
            } else {
                self.len.fetch_add(1, Ordering::Release);
                return;
            }
        }
    }

    // TODO: get_or_insert

    pub fn get(&self, key: &K) -> Option<&V>
    where
        K: PartialEq,
    {
        self.iter().find(|(k, _)| *k == key).map(|(_, v)| v)
    }

    pub fn len(&self) -> usize {
        self.len.load(Ordering::Acquire)
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn iter(&self) -> impl Iterator<Item = (&K, &V)> {
        let mut cur = &self.head;
        core::iter::from_fn(move || {
            let cur_ptr = cur.load(Ordering::Acquire);
            if cur_ptr.is_null() {
                return None;
            }

            let cur_node: &AtomicMapNode<K, V> = unsafe { &*cur_ptr };
            cur = &cur_node.next;
            Some((&cur_node.key, &cur_node.value))
        })
    }

    pub fn keys(&self) -> impl Iterator<Item = &K> {
        self.iter().map(|(k, _)| k)
    }

    pub fn values(&self) -> impl Iterator<Item = &V> {
        self.iter().map(|(_, v)| v)
    }
}

impl<K, V> Drop for AtomicMap<K, V> {
    fn drop(&mut self) {
        let mut head = self.head.load(Ordering::Relaxed);

        loop {
            if head.is_null() {
                return;
            }

            let node = unsafe { Box::from_raw(head) };
            head = node.next.load(Ordering::Relaxed);
        }
    }
}

//

#[cfg(test)]
mod tests {

    #[cfg(loom)]
    #[test]
    fn test_drop() {
        use alloc::sync::Arc;

        use crate::AtomicMap;

        loom::model(|| {
            let map = AtomicMap::new();

            let v0 = Arc::new(());

            assert_eq!(Arc::strong_count(&v0), 1);
            map.insert(0, v0.clone());
            assert_eq!(Arc::strong_count(&v0), 2);
            map.insert(0, v0.clone());
            assert_eq!(Arc::strong_count(&v0), 3);
            drop(map);
            assert_eq!(Arc::strong_count(&v0), 1);
        });
    }
}
