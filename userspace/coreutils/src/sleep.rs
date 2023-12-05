use anyhow::{anyhow, Result};
use libstd::sys::nanosleep;

//

pub fn cmd<'a>(mut args: impl Iterator<Item = &'a str>) -> Result<()> {
    let a1 = args
        .next()
        .ok_or_else(|| anyhow!("expected at least one argument"))?;

    let s = a1.ends_with('s');
    let m = a1.ends_with('m');
    let h = a1.ends_with('h');
    let d = a1.ends_with('d');

    let n = if s || m || h || d {
        &a1[..a1.len() - 1]
    } else {
        a1
    };
    let mut n = n
        .parse::<f64>()
        .map_err(|_| anyhow!("invalid time interval `{a1}`"))?;

    _ = s;
    if m {
        n *= 60.0;
    } else if h {
        n *= 60.0 * 60.0;
    } else if d {
        n *= 60.0 * 60.0 * 24.0;
    }

    let nanos = (n * 1_000_000_000.0) as u64;
    nanosleep(nanos);

    Ok(())
}
