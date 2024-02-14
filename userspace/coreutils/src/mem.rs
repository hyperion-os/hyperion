use alloc::string::String;
use core::ops::Deref;

use anyhow::{anyhow, Result};
use hyperion_num_postfix::NumberPostfix;
use libstd::println;

//

pub fn cmd<'a>(_: impl Iterator<Item = &'a str>) -> Result<()> {
    let (total, _, used) = read_meminfo()?;

    let p = used as f64 / total as f64 * 100.0;
    let used = used.postfix_binary();
    let total = total.postfix_binary();

    println!("Mem:\n - total: {total}B\n - used: {used}B ({p:3.1}%)");

    Ok(())
}

pub fn read_meminfo() -> Result<(usize, usize, usize)> {
    let mut buf = String::new();
    let meminfo = super::read_file_map(&mut buf, "/proc/meminfo")?;

    let get_ent = |name: &str| -> Result<usize> {
        let value = meminfo.get(name).unwrap().deref();

        let (value, kb) = value
            .split_once(' ')
            .map(|(num, kb)| (num, Some(kb)))
            .unwrap_or((value, None));

        let mut num = value.parse::<usize>().map_err(|err| anyhow!("{err}"))?;
        if kb == Some("kb") {
            num *= 0x400;
        }

        Ok(num)
    };

    let total = get_ent("MemTotal")?;
    let free = get_ent("MemFree")?;
    let used = total - free;

    Ok((total, free, used))
}
