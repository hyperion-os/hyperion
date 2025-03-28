use alloc::{sync::Arc, vec::Vec};
use core::{
    hash::{Hash, Hasher},
    marker::PhantomData,
    mem,
    ops::{Deref, DerefMut},
};

use hyperion_random::Rng;

use crate::{
    lazy::Once,
    lock::{Mutex, MutexGuard},
};

//

pub struct LazyHasher {
    inner: Once<DefaultHasher>,
}

impl LazyHasher {
    pub const fn new() -> Self {
        Self { inner: Once::new() }
    }

    pub async fn hash<K: Hash>(&self, key: &K) -> u64 {
        self.inner
            .call_once(async { DefaultHasher::new() })
            .await
            .hash(key)
    }
}

impl Default for LazyHasher {
    fn default() -> Self {
        Self::new()
    }
}

//

pub const LOAD_FACTOR_NUMERATOR: usize = 75;
pub const LOAD_FACTOR_DENOMINATOR: usize = 100;

/// `SEGMENTS` has to be a power of 2
pub struct AsyncHashMap<K, V, const SEGMENTS: usize = 32> {
    segments: [Mutex<Segment<K, V, SEGMENTS>>; 32],
    hasher: LazyHasher,
}

impl<K, V, const SEGMENTS: usize> AsyncHashMap<K, V, SEGMENTS> {
    pub const fn new() -> Self {
        Self {
            segments: [const { Mutex::new(Segment::new()) }; 32],
            hasher: LazyHasher::new(),
        }
    }

    async fn segment(&self, hash: u64) -> MutexGuard<'_, Segment<K, V, SEGMENTS>> {
        let id = HashId::<SEGMENTS>::segment_id(hash);
        self.segments[id].lock().await
    }
}

impl<K: Hash + Eq, V> AsyncHashMap<K, V> {
    pub async fn get(&self, key: &K) -> Option<Ref<K, V>> {
        let hash = self.hasher.hash(key).await;

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
        let hash = self.hasher.hash(&key).await;

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

    pub async fn remove(&self, key: &K) -> Option<Ref<K, V>> {
        let hash = self.hasher.hash(&key).await;

        // find the correct segment, each segment is individually locked

        Some(self.segment(hash).await.remove(hash, key)?.lock().await)
    }

    pub async fn entry(&self, key: K) -> Entry<'_, K, V> {
        let hash = self.hasher.hash(&key).await;

        let mut bucket = MutexGuard::map(self.segment(hash).await, |buckets| {
            if buckets.buckets.is_empty() {
                buckets.buckets.push(Bucket::new());
            }
            buckets.bucket(hash)
        });

        if let Some(item) = bucket.find(&key).cloned() {
            Entry::Occupied(OccupiedEntry {
                item: item.lock().await,
                _p: PhantomData,
            })
        } else {
            Entry::Vacant(VacantEntry { bucket, hash, key })
        }
    }
}

impl<K, V> Default for AsyncHashMap<K, V> {
    fn default() -> Self {
        Self::new()
    }
}

//

pub enum Entry<'a, K, V> {
    Occupied(OccupiedEntry<'a, K, V>),
    Vacant(VacantEntry<'a, K, V>),
}

pub struct OccupiedEntry<'a, K, V> {
    item: Ref<K, V>,
    _p: PhantomData<&'a ()>,
}

impl<K, V> OccupiedEntry<'_, K, V> {
    pub fn get(&self) -> &V {
        &self.item
    }

    pub fn get_mut(&mut self) -> &mut V {
        &mut self.item
    }

    pub fn insert(&mut self, mut val: V) -> V {
        let old = self.get_mut();
        mem::swap(old, &mut val);
        val
    }

    #[deprecated = "unimplemented"]
    pub fn remove(self) -> V {
        unimplemented!()
    }

    pub fn inner(self) -> Ref<K, V> {
        self.item
    }
}

pub struct VacantEntry<'a, K, V> {
    hash: u64,
    key: K,
    bucket: MutexGuard<'a, Bucket<K, V>>,
}

impl<K: Eq + Hash, V> VacantEntry<'_, K, V> {
    pub async fn insert(mut self, val: V) -> Ref<K, V> {
        let item = Arc::new(Item {
            hash: self.hash,
            key: self.key,
            val: Mutex::new(val),
        });
        let result = item.clone().lock().await;
        self.bucket.insert(item);
        result
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

struct Segment<K, V, const SEGMENTS: usize> {
    buckets: Vec<Bucket<K, V>>,
    count: usize,
}

impl<K, V, const SEGMENTS: usize> Segment<K, V, SEGMENTS> {
    const fn new() -> Self {
        Self {
            buckets: Vec::new(),
            count: 0,
        }
    }

    fn bucket(&mut self, hash: u64) -> &mut Bucket<K, V> {
        let id = HashId::<SEGMENTS>::bucket_id(hash, self.buckets.len());
        &mut self.buckets[id]
    }
}

impl<K: Eq, V, const SEGMENTS: usize> Segment<K, V, SEGMENTS> {
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

    fn remove(&mut self, hash: u64, key: &K) -> Option<Arc<Item<K, V>>> {
        if self.buckets.is_empty() {
            return None;
        }
        self.bucket(hash).remove(key)
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

    fn remove(&mut self, key: &K) -> Option<Arc<Item<K, V>>> {
        let first = self.item.as_mut()?;
        if first.matches(key) {
            let removed = first.clone();
            self.item = self.list.pop();
            return Some(removed);
        }

        Self::remove_slow(&mut self.list, key)
    }

    // remove with a hash collision
    #[cold]
    fn remove_slow(list: &mut Vec<Arc<Item<K, V>>>, key: &K) -> Option<Arc<Item<K, V>>> {
        let index = list.iter_mut().position(|item| item.matches(key))?;
        Some(list.remove(index))
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

struct HashId<const SEGMENTS: usize>;

impl<const SEGMENTS: usize> HashId<SEGMENTS> {
    pub const SEGMENT_MASK: usize = SEGMENTS - 1;
    pub const SEGMENT_SHIFT: u32 = SEGMENTS.ilog2();
    const _C: () = assert!(SEGMENTS.next_power_of_two() == SEGMENTS);

    fn segment_id(hash: u64) -> usize {
        (hash as usize) & Self::SEGMENT_MASK
    }

    fn bucket_id(hash: u64, n_buckets: usize) -> usize {
        ((hash >> Self::SEGMENT_SHIFT) as usize) % n_buckets
    }
}
