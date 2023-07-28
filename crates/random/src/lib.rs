#![no_std]

//

extern crate alloc;

use core::sync::atomic::{AtomicBool, Ordering};

use rand::{distributions::Standard, prelude::Distribution};
pub use rand::{CryptoRng, Fill, Rng, RngCore, SeedableRng};
use rand_chacha::{ChaCha20Rng, ChaCha8Rng, ChaChaRng};
use spin::{Mutex, Once};

//

pub fn provide_entropy(data: &[u8]) {
    get_entropy().feed(data)
}

pub fn next_secure_rng() -> Option<ChaCha20Rng> {
    Some(ChaCha20Rng::from_seed(get_entropy().gen_secure()?))
}

pub fn next_fast_rng() -> ChaCha8Rng {
    ChaCha8Rng::from_seed(get_entropy().gen_fast())
}

//

fn get_entropy() -> &'static EntropyCollector {
    ENTROPY.call_once(EntropyCollector::new)
}

//

static ENTROPY: Once<EntropyCollector> = Once::new();

//

struct EntropyCollector {
    hasher: Mutex<blake3::Hasher>,
    rng: Mutex<ChaChaRng>,
    is_insecure: AtomicBool,
}

//

impl EntropyCollector {
    fn new() -> Self {
        let mut hasher = blake3::Hasher::new();

        let seed = Self::feed_inner(&mut hasher, hyperion_macros::build_rev!().as_bytes());

        Self {
            hasher: Mutex::new(hasher),
            rng: Mutex::new(ChaChaRng::from_seed(seed)),
            is_insecure: AtomicBool::new(true),
        }
    }

    fn gen_fast<T>(&self) -> T
    where
        Standard: Distribution<T>,
    {
        let mut rng = self.rng.lock();

        if self.is_insecure.load(Ordering::Acquire) {
            hyperion_log::error!("Using insecure PRNG seed");
        }

        rng.gen()
    }

    fn gen_secure<T>(&self) -> Option<T>
    where
        Standard: Distribution<T>,
    {
        let mut rng = self.rng.lock();

        if self.is_insecure.load(Ordering::Acquire) {
            hyperion_log::error!("Using insecure PRNG seed");
            return None;
        }

        Some(rng.gen())
    }

    fn feed(&self, data: &[u8]) {
        // lock `rng` before `sha` to avoid some race condition shenanigans
        let mut rng = self.rng.lock();
        let mut hasher = self.hasher.lock();

        let seed = Self::feed_inner(&mut hasher, data);

        *rng = ChaChaRng::from_seed(seed);

        self.is_insecure.store(false, Ordering::Release);
    }

    fn feed_inner(hasher: &mut blake3::Hasher, data: &[u8]) -> [u8; 32] {
        hasher.update(data);

        let digest = hasher.finalize();
        *digest.as_bytes()
    }
}
