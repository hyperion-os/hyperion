use alloc::format;

use anyhow::{anyhow, Result};
use hyperion_num_postfix::NumberPostfix;
use libstd::{fs::Dir, print, println};

//

pub fn cmd<'a>(mut args: impl Iterator<Item = &'a str>) -> Result<()> {
    let a1 = args
        .next()
        .ok_or_else(|| anyhow!("expected at least one argument"))?;

    let mut dir = Dir::open(a1).unwrap();

    println!("mode size name");
    while let Some(entry) = dir.next_entry() {
        let size = entry.size.postfix_binary();
        let size_n = size.into_inner();
        let size_scale = size.scale();

        let size = format!("{size_n}{size_scale}B");

        if entry.is_dir {
            print!("d       - ");
        } else {
            print!("f {size: >7} ");
        }

        println!("{}", entry.file_name);
    }

    Ok(())
}
