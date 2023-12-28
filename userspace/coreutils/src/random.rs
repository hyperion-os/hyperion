use alloc::vec::Vec;
use core::{fmt, str::FromStr};

use anyhow::{anyhow, Result};
use libstd::{fs::File, io::Read, println};
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;

//

pub fn cmd<'a>(args: impl Iterator<Item = &'a str>) -> Result<()> {
    let args: Vec<&str> = args.collect();
    match args[..] {
        [] => {
            random(None, 0, 1, 32767);
        }
        [seed] => {
            let seed = parse(seed, "seed")?;
            random(Some(seed), 0, 1, i16::MAX as _);
        }
        [start, end] => {
            let start = parse(start, "start")?;
            let end = parse(end, "end")?;
            random(None, start, 1, end);
        }
        [start, step, end] => {
            let start = parse(start, "start")?;
            let step = parse(step, "step")?;
            let end = parse(end, "end")?;
            random(None, start, step, end);
        }
        _ => return Err(anyhow!("too many arguments")),
    }

    Ok(())
}

fn parse<T>(s: &str, var: &str) -> Result<T>
where
    T: FromStr,
    T::Err: fmt::Display,
{
    s.parse::<T>()
        .map_err(|err| anyhow!("failed to parse {var}: {err}"))
}

fn random(seed: Option<u64>, start: isize, step: isize, end: isize) {
    let mut rng = rng(seed);
    let val = (rng.gen_range(start..=end) / step * step).clamp(start, end);
    println!("{val}");
}

fn rng(seed: Option<u64>) -> ChaCha8Rng {
    let seed = seed.unwrap_or_else(|| {
        let mut seed_bytes = [0u8; 8];
        let mut n = 0;

        File::open("/dev/urandom")
            .unwrap()
            .read_exact(&mut seed_bytes, &mut n)
            .unwrap();

        assert_eq!(n, seed_bytes.len());

        u64::from_ne_bytes(seed_bytes)
    });

    ChaCha8Rng::seed_from_u64(seed)
}
