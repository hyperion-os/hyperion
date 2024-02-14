use anyhow::{anyhow, Result};
use libstd::fs::create_dir;

//

pub fn cmd<'a>(args: impl Iterator<Item = &'a str>) -> Result<()> {
    for dir in args {
        create_dir(dir).map_err(|err| anyhow!("{err}"))?;
    }

    Ok(())
}
