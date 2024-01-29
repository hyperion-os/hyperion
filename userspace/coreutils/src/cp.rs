use anyhow::{anyhow, Result};
use libstd::{
    fs::{File, OpenOptions},
    io::{Read, Write},
};

//

pub fn cmd<'a>(mut args: impl Iterator<Item = &'a str>) -> Result<()> {
    let from_path = args
        .next()
        .ok_or_else(|| anyhow!("expected at least two arguments"))?;

    let to_path = args
        .next()
        .ok_or_else(|| anyhow!("expected at least two arguments"))?;

    // TODO: copy dir
    // TODO: copy multiple

    let mut from: File = OpenOptions::new()
        .read(true)
        .create(false)
        .open(from_path)
        .map_err(|err| anyhow!("cannot open input file `{from_path}`: {err}"))?;

    let mut to: File = OpenOptions::new()
        .write(true)
        .create(true)
        .open(to_path)
        .map_err(|err| anyhow!("cannot open output file `{to_path}`: {err}"))?;

    let mut buf = [0u8; 0x4000];
    loop {
        let n: usize = from
            .read(&mut buf)
            .map_err(|err| anyhow!("cannot read from `{from_path}`: {err}"))?;
        if n == 0 {
            break;
        }
        to.write_all(&buf[..n])
            .map_err(|err| anyhow!("cannot write to `{to_path}`: {err}"))?;
    }

    Ok(())
}
