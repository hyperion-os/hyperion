#![no_std]

//

extern crate alloc;

use rand_chacha::{
    rand_core::{RngCore, SeedableRng},
    ChaCha8Rng,
};
use spin::{Mutex, Once};

//

pub fn provide_entropy(data: &[u8]) {
    get_entropy().feed(data)
}

pub fn rand() -> u64 {
    get_entropy().with_rng(|rng| rng.next_u64())
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
    rng: Mutex<(ChaCha8Rng, bool)>,
}

//

impl EntropyCollector {
    pub fn new() -> Self {
        Self {
            sha: Mutex::new([0; 32]),
            rng: Mutex::new((ChaCha8Rng::from_seed([0; 32]), true)),
        }
    }

    pub fn with_rng<T>(&self, f: impl FnOnce(&mut ChaCha8Rng) -> T) -> T {
        let mut rng = self.rng.lock();

        if rng.1 {
            hyperion_log::error!("Using insecure PRNG seed");
        }

        f(&mut rng.0)
    }

    pub fn feed(&self, data: &[u8]) {
        let mut sha = self.sha.lock();
        let mut hasher = blake3::Hasher::new();

        hasher.update(&*sha);
        hasher.update(data);

        let digest = hasher.finalize();
        let seed = digest.as_bytes();

        sha.copy_from_slice(seed);

        *self.rng.lock() = (ChaCha8Rng::from_seed(*seed), false);
    }
}
