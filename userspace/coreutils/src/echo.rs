use anyhow::Result;
use libstd::{print, println};

//

pub fn cmd<'a>(args: impl Iterator<Item = &'a str>) -> Result<()> {
    for arg in args {
        print!("{arg} ");
    }

    println!();

    Ok(())
}
