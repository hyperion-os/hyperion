use core::{
    ptr,
    sync::atomic::{AtomicPtr, AtomicUsize, Ordering},
};

use alloc::boxed::Box;

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
    pub const fn new() -> Self {
        Self {
            len: AtomicUsize::new(0),
            head: AtomicPtr::new(ptr::null_mut()),
        }
    }

    pub fn insert(&self, key: K, value: V) {
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

    pub fn get(&self, key: &K) -> Option<&V>
    where
        K: PartialEq,
    {
        self.iter().find(|(k, _)| *k == key).map(|(_, v)| v)
    }

    pub fn len(&self) -> usize {
        self.len.load(Ordering::Acquire)
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

// TODO: drop
