use alloc::{format, vec::Vec};

use anyhow::{anyhow, Result};
use hyperion_num_postfix::NumberPostfix;
use libstd::{
    fs::{Dir, DirEntry},
    print, println,
};

//

pub fn cmd<'a>(mut args: impl Iterator<Item = &'a str>) -> Result<()> {
    let a1 = args.next().unwrap_or("/"); // TODO: cwd
                                         // .ok_or_else(|| anyhow!("expected at least one argument"))?;

    let mut entries: Vec<DirEntry> = Dir::open(a1).map_err(|err| anyhow!("{err}"))?.collect();
    entries.sort_by(|a, b| {
        let cmp = (!a.is_dir).cmp(&(!b.is_dir));
        if cmp.is_ne() {
            return cmp;
        }

        let cmp = a.file_name.cmp(&b.file_name);
        if cmp.is_ne() {
            return cmp;
        }

        a.size.cmp(&b.size)
    });

    println!("mode size name");
    for entry in entries {
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
