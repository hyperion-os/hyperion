#![no_std]

//

extern crate alloc;

pub use rand::{CryptoRng, Fill, Rng, RngCore, SeedableRng};
use rand_chacha::{ChaCha20Rng, ChaCha8Rng, ChaChaRng};
use spin::{Mutex, Once};

//

pub fn provide_entropy(data: &[u8]) {
    get_entropy().feed(data)
}

/* pub fn next_u64() -> u64 {
    get_entropy().with_rng(|rng| rng.next_u64())
}

pub fn next_fast_seed() -> [u8; 32] {
    get_entropy().with_rng(|rng| {
        let mut bytes = [0; 32];
        rng.fill_bytes(&mut bytes);
        bytes
    })
}

pub fn next_secure_seed() -> [u8; 32] {
    get_entropy().with_rng(|rng| {
        let mut bytes = [0; 32];
        rng.fill_bytes(&mut bytes);
        bytes
    })
} */

pub fn next_secure_rng() -> Option<ChaCha20Rng> {
    get_entropy().with_secure_rng(|rng| {
        let mut bytes = [0; 32];
        rng.fill_bytes(&mut bytes);
        ChaCha20Rng::from_seed(bytes)
    })
}

pub fn next_fast_rng() -> ChaCha8Rng {
    get_entropy().with_rng(|rng| {
        let mut bytes = [0; 32];
        rng.fill_bytes(&mut bytes);
        ChaCha8Rng::from_seed(bytes)
    })
}

//

fn get_entropy() -> &'static EntropyCollector {
    ENTROPY.call_once(EntropyCollector::new)
}

//

static ENTROPY: Once<EntropyCollector> = Once::new();

//

struct EntropyCollector {
    sha: Mutex<[u8; 32]>,
    rng: Mutex<(ChaChaRng, bool)>,
}

//

impl EntropyCollector {
    pub fn new() -> Self {
        Self {
            sha: Mutex::new([0; 32]),
            rng: Mutex::new((ChaChaRng::from_seed([0; 32]), true)),
        }
    }

    pub fn with_rng<T>(&self, f: impl FnOnce(&mut ChaChaRng) -> T) -> T {
        let mut rng = self.rng.lock();

        if rng.1 {
            hyperion_log::error!("Using insecure PRNG seed");
        }

        f(&mut rng.0)
    }

    pub fn with_secure_rng<T>(&self, f: impl FnOnce(&mut ChaChaRng) -> T) -> Option<T> {
        let mut rng = self.rng.lock();

        if rng.1 {
            hyperion_log::error!("Using insecure PRNG seed");
            return None;
        }

        Some(f(&mut rng.0))
    }

    pub fn feed(&self, data: &[u8]) {
        let mut sha = self.sha.lock();
        let mut hasher = blake3::Hasher::new();

        hasher.update(&*sha);
        hasher.update(data);

        let digest = hasher.finalize();
        let seed = digest.as_bytes();

        sha.copy_from_slice(seed);

        *self.rng.lock() = (ChaChaRng::from_seed(*seed), false);
    }
}
