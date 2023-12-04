use anyhow::{anyhow, Result};
use libstd::fs::OpenOptions;

//

pub fn cmd<'a>(mut args: impl Iterator<Item = &'a str>) -> Result<()> {
    let a1 = args
        .next()
        .ok_or_else(|| anyhow!("expected at least one argument"))?;

    let _ = OpenOptions::new()
        .write(true)
        .create(true)
        .open(a1)
        .map_err(|err| anyhow!("`{a1}`: {err}"))?;

    Ok(())
}
