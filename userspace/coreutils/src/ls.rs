use anyhow::{anyhow, Result};

//

pub fn cmd<'a>(_: impl Iterator<Item = &'a str>) -> Result<()> {
    Err(anyhow!("TODO:"))

    // let a1 = args
    //     .next()
    //     .ok_or_else(|| anyhow!("expected at least one argument"))?;

    // let s: std::fs::ReadDir = std::fs::read_dir(a1).unwrap();
    // s.into_iter();

    // let file = File::open(a1).map_err(|err| anyhow!("`{a1}`: {err}"))?;
    // _ = file;

    // println!();

    // Ok(())
}
