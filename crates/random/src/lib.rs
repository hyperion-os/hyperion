#![no_std]

//

extern crate alloc;

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
    sha: Mutex<[u8; 32]>,
    rng: Mutex<(ChaChaRng, bool)>,
}

//

impl EntropyCollector {
    fn new() -> Self {
        Self {
            sha: Mutex::new(INIT_DATA),
            rng: Mutex::new((ChaChaRng::from_seed(INIT_DATA), true)),
        }
    }

    fn gen_fast<T>(&self) -> T
    where
        Standard: Distribution<T>,
    {
        let mut rng = self.rng.lock();

        if rng.1 {
            hyperion_log::error!("Using insecure PRNG seed");
        }

        rng.0.gen()
    }

    fn gen_secure<T>(&self) -> Option<T>
    where
        Standard: Distribution<T>,
    {
        let mut rng = self.rng.lock();

        if rng.1 {
            hyperion_log::error!("Using insecure PRNG seed");
            return None;
        }

        Some(rng.0.gen())
    }

    fn feed(&self, data: &[u8]) {
        // lock `rng` before `sha` to avoid some race condition shenanigans
        let mut rng = self.rng.lock();
        let mut sha = self.sha.lock();
        let mut hasher = blake3::Hasher::new();

        hasher.update(&*sha);
        hasher.update(data);

        let digest = hasher.finalize();
        let seed = digest.as_bytes();

        sha.copy_from_slice(seed);

        *rng = (ChaChaRng::from_seed(*seed), false);
    }
}

//

const INIT_DATA: [u8; 32] = {
    let mut buf = [0; 32];
    let rev = hyperion_macros::build_rev!().as_bytes();

    let mut i = 0usize;
    while i < 32 {
        buf[i] = rev[i];
        i += 1;
    }

    buf
};
