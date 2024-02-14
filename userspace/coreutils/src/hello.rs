use anyhow::Result;
use libstd::println;

//

pub fn cmd<'a>(_: impl Iterator<Item = &'a str>) -> Result<()> {
    // this exists just so that I can quickly add more tools by copy/pasting this file
    println!("Hello, world!");
    Ok(())
}
