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
    Some(ChaCha20Rng::from_seed(get_entropy().gen().ok()?))
}

pub fn next_fast_rng() -> ChaCha8Rng {
    ChaCha8Rng::from_seed(get_entropy().gen().unwrap_or_else(|insecure| insecure.0))
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

struct InsecureError<T>(T);

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

    fn gen<T>(&self) -> Result<T, InsecureError<T>>
    where
        Standard: Distribution<T>,
    {
        let is_insecure = self.is_insecure.load(Ordering::Acquire);
        let val = self.rng.lock().gen();

        if is_insecure {
            Err(InsecureError(val))
        } else {
            Ok(val)
        }
    }

    fn feed(&self, data: &[u8]) {
        // lock `rng` before `sha` to avoid some race condition shenanigans
        let mut rng = self.rng.lock();
        let mut hasher = self.hasher.lock();

        let seed = Self::feed_inner(&mut hasher, data);

        *rng = ChaChaRng::from_seed(seed);

        // TODO: "secure" depends
        // now it is "secure" when literally any data is fed to the collector
        self.is_insecure.store(false, Ordering::Release);
    }

    fn feed_inner(hasher: &mut blake3::Hasher, data: &[u8]) -> [u8; 32] {
        hasher.update(data);

        let digest = hasher.finalize();
        *digest.as_bytes()
    }
}
