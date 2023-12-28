use alloc::vec::Vec;
use core::fmt;

use anyhow::{anyhow, Result};
use libstd::{
    fs::{File, STDIN},
    io::Read,
    print, println,
};

//

pub fn cmd<'a>(args: impl Iterator<Item = &'a str>) -> Result<()> {
    let mut stdin = STDIN.lock();
    let stdin = stdin.get_mut();

    let mut files = args
        .map(|path| {
            File::open(path)
                .map(|file| (path, file))
                .map_err(|err| anyhow!("`{path}`: {err}"))
        })
        .collect::<Result<Vec<_>>>()?;
    let argc = files.len();

    let mut file_iter = [("<stdin>", stdin)]
        .into_iter()
        .chain(files.iter_mut().map(|(path, file)| (*path, file)));

    if argc != 0 {
        file_iter.next(); // skip stdio
    }

    for (path, file) in file_iter {
        let mut buf = [0u8; 512];
        loop {
            let len = file
                .read(&mut buf)
                .map_err(|err| anyhow!("`{path}`: {err}"))?;
            let buf = &buf[..len];

            if len == 0 {
                break;
            }

            print!("{}", Bytes(buf));
        }
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
