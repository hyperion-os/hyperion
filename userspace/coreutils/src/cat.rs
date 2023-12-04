use core::fmt;

use anyhow::{anyhow, Result};
use libstd::{fs::File, print, println};

//

pub fn cmd<'a>(mut args: impl Iterator<Item = &'a str>) -> Result<()> {
    let a1 = args
        .next()
        .ok_or_else(|| anyhow!("expected at least one argument"))?;

    let file = File::open(a1).map_err(|err| anyhow!("`{a1}`: {err}"))?;

    let mut buf = [0u8; 512];
    loop {
        let len = file
            .read(&mut buf)
            .map_err(|err| anyhow!("`{a1}`: {err}"))?;
        let buf = &buf[..len];

        if len == 0 {
            break;
        }

        print!("{}", Bytes(buf));
    }

    println!();

    Ok(())
}

//

struct Bytes<'a>(&'a [u8]);

impl<'a> fmt::Display for Bytes<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        for byte in self.0 {
            if byte.is_ascii() {
                write!(f, "{}", *byte as char)?;
            }
            // write!(f, "{byte:x} ")?;
        }
        Ok(())
    }
}
