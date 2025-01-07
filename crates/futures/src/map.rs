use alloc::{sync::Arc, vec::Vec};
use core::{
    hash::{Hash, Hasher},
    mem,
    ops::{Deref, DerefMut},
};

use hyperion_random::Rng;

use crate::{
    lazy::Once,
    lock::{Mutex, MutexGuard},
};

//

pub const LOAD_FACTOR_NUMERATOR: usize = 75;
pub const LOAD_FACTOR_DENOMINATOR: usize = 100;

/// has to be a power of 2
pub const SEGMENTS: usize = 32;
pub const SEGMENT_MASK: usize = SEGMENTS - 1;
pub const SEGMENT_SHIFT: u32 = SEGMENTS.ilog2();

const _: () = assert!(SEGMENTS.next_power_of_two() == SEGMENTS);

//

type Segments<K, V> = [Mutex<Segment<K, V>>; SEGMENTS];

pub struct AsyncHashMap<K, V> {
    segments: Segments<K, V>,
    hasher: Once<DefaultHasher>,
}

impl<K, V> AsyncHashMap<K, V> {
    pub const fn new() -> Self {
        Self {
            segments: [const { Mutex::new(Segment::new()) }; 32],
            hasher: Once::new(),
        }
    }

    async fn hasher(&self) -> &DefaultHasher {
        self.hasher.call_once(async { DefaultHasher::new() }).await
    }

    async fn segment(&self, hash: u64) -> MutexGuard<'_, Segment<K, V>> {
        self.segments[segment_id(hash)].lock().await
    }
}

impl<K: Hash + Eq, V> AsyncHashMap<K, V> {
    pub async fn get(&self, key: &K) -> Option<Ref<K, V>> {
        let hash = self.hasher().await.hash(key);

        Some(
            self.segment(hash)
                .await
                .find(hash, key)?
                .clone()
                .lock()
                .await,
        )
    }

    pub async fn insert(&self, key: K, val: V) -> bool {
        let hash = self.hasher().await.hash(&key);

        // find the correct segment, each segment is individually locked
        self.segment(hash)
            .await
            .insert(Arc::new(Item {
                hash,
                key,
                val: Mutex::new(val),
            }))
            .is_some()
    }
}

impl<K, V> Default for AsyncHashMap<K, V> {
    fn default() -> Self {
        Self::new()
    }
}

//

struct DefaultHasher {
    init_state: u128,
}

impl DefaultHasher {
    fn new() -> Self {
        Self {
            init_state: hyperion_random::next_fast_rng().gen(),
        }
    }

    fn hash<T: Hash>(&self, item: &T) -> u64 {
        struct NewDefaultHasher(blake3::Hasher);

        impl Hasher for NewDefaultHasher {
            fn finish(&self) -> u64 {
                let mut num = [0u8; 8];
                self.0.finalize_xof().fill(&mut num);
                u64::from_ne_bytes(num)
            }

            fn write(&mut self, bytes: &[u8]) {
                self.0.update(bytes);
            }
        }

        let mut hasher = NewDefaultHasher(blake3::Hasher::new());
        self.init_state.hash(&mut hasher);
        item.hash(&mut hasher);
        hasher.finish()
    }
}

//

struct Segment<K, V> {
    buckets: Vec<Bucket<K, V>>,
    count: usize,
}

impl<K, V> Segment<K, V> {
    const fn new() -> Self {
        Self {
            buckets: Vec::new(),
            count: 0,
        }
    }

    fn bucket(&mut self, hash: u64) -> &mut Bucket<K, V> {
        let n_buckets = self.buckets.len();
        &mut self.buckets[bucket_id(hash, n_buckets)]
    }
}

impl<K: Eq, V> Segment<K, V> {
    fn insert(&mut self, item: Arc<Item<K, V>>) -> Option<Arc<Item<K, V>>> {
        if self.count * LOAD_FACTOR_DENOMINATOR >= self.buckets.len() * LOAD_FACTOR_NUMERATOR {
            // the first insert always resizes
            self.resize();
        }

        self.insert_noresize(item)
    }

    fn insert_noresize(&mut self, item: Arc<Item<K, V>>) -> Option<Arc<Item<K, V>>> {
        let result = self.bucket(item.hash).insert(item);
        if result.is_none() {
            self.count += 1;
        }
        result
    }

    #[cold]
    fn resize(&mut self) {
        hyperion_log::debug!("resizing");
        let new_len = (self.buckets.len() + 1).next_power_of_two();

        let mut new_self = Self {
            buckets: (0..new_len).map(|_| Bucket::new()).collect(),
            count: 0,
        };
        mem::swap(self, &mut new_self);

        for item in new_self.drain() {
            self.insert_noresize(item);
        }
    }

    fn find(&mut self, hash: u64, key: &K) -> Option<&mut Arc<Item<K, V>>> {
        if self.buckets.is_empty() {
            return None;
        }
        self.bucket(hash).find(key)
    }

    fn drain(self) -> impl Iterator<Item = Arc<Item<K, V>>> {
        self.buckets.into_iter().flat_map(|bucket| bucket.drain())
    }
}

//

struct Bucket<K, V> {
    item: Option<Arc<Item<K, V>>>,
    list: Vec<Arc<Item<K, V>>>,
}

impl<K, V> Bucket<K, V> {
    pub const fn new() -> Self {
        Self {
            item: None,
            list: Vec::new(),
        }
    }
}

impl<K: Eq, V> Bucket<K, V> {
    fn insert(&mut self, item: Arc<Item<K, V>>) -> Option<Arc<Item<K, V>>> {
        if self.item.is_none() {
            self.item = Some(item);
            return None;
        }

        self.insert_slow(item)
    }

    // insert with a hash collision
    #[cold]
    fn insert_slow(&mut self, mut item: Arc<Item<K, V>>) -> Option<Arc<Item<K, V>>> {
        if let Some(slot) = self.find(&item.key) {
            mem::swap(slot, &mut item);
            return Some(item);
        }

        self.list.push(item);
        None
    }

    fn find(&mut self, key: &K) -> Option<&mut Arc<Item<K, V>>> {
        let first = self.item.as_mut()?;
        if first.matches(key) {
            return Some(first);
        }

        Self::find_slow(&mut self.list, key)
    }

    // find with a hash collision
    #[cold]
    fn find_slow<'a>(list: &'a mut [Arc<Item<K, V>>], key: &K) -> Option<&'a mut Arc<Item<K, V>>> {
        list.iter_mut().find(|item| item.matches(key))
    }

    fn drain(self) -> impl Iterator<Item = Arc<Item<K, V>>> {
        self.item.into_iter().chain(self.list)
    }
}

//

struct Item<K, V> {
    hash: u64,
    key: K,
    val: Mutex<V>,
}

impl<K, V> Item<K, V> {
    async fn lock(self: Arc<Self>) -> Ref<K, V> {
        Ref::lock(self).await
    }
}

impl<K: Eq, V> Item<K, V> {
    fn matches(&self, k: &K) -> bool {
        &self.key == k
    }
}

//

pub struct Ref<K, V> {
    item: Arc<Item<K, V>>,
}

impl<K, V> Ref<K, V> {
    async fn lock(item: Arc<Item<K, V>>) -> Self {
        mem::forget(item.val.lock());
        Self { item }
    }
}

impl<K, V> Deref for Ref<K, V> {
    type Target = V;

    fn deref(&self) -> &Self::Target {
        // SAFETY: Ref keeps the mutex locked without the guard
        unsafe { self.item.val.get_force() }
    }
}

impl<K, V> DerefMut for Ref<K, V> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        // SAFETY: Ref keeps the mutex locked without the guard
        unsafe { self.item.val.get_mut_force() }
    }
}

impl<K, V> Drop for Ref<K, V> {
    fn drop(&mut self) {
        // SAFETY: Ref keeps the mutex locked without the guard
        unsafe { self.item.val.unlock() };
    }
}

//

fn segment_id(hash: u64) -> usize {
    (hash as usize) & SEGMENT_MASK
}

fn bucket_id(hash: u64, buckets: usize) -> usize {
    (hash >> SEGMENT_SHIFT) as usize % buckets
}
