#![no_std]

//

extern crate alloc;

mod cat;
mod cp;
mod date;
mod echo;
mod hello;
mod ls;
mod mem;
mod mkdir;
mod nproc;
mod ps;
mod random;
mod sleep;
mod tail;
mod top;
mod touch;

//

use alloc::{boxed::Box, collections::BTreeMap, string::String};

use anyhow::{anyhow, Result};
use libstd::{env, eprintln, fs::File, io::BufReader, println, sys::exit};

//

fn main() {
    let mut args = env::args();
    let cmd = args.next().expect("arg 0 should always be there");

    let cmd = cmd.rsplit_once('/').map(|(_, rhs)| rhs).unwrap_or(cmd);

    let result = match cmd {
        "cat" => cat::cmd(args),
        "coreutils" => crate::cmd(),
        "cp" => cp::cmd(args),
        "date" => date::cmd(args),
        "echo" => echo::cmd(args),
        "hello" => hello::cmd(args),
        "ls" => ls::cmd(args),
        "mem" => mem::cmd(args),
        "mkdir" => mkdir::cmd(args),
        "nproc" => nproc::cmd(args),
        "ps" => ps::cmd(args),
        "random" => random::cmd(args),
        "sleep" => sleep::cmd(args),
        "tail" => tail::cmd(args),
        "top" => top::cmd(args),
        "touch" => touch::cmd(args),
        _ => {
            eprintln!("`{cmd}` is not part of hyperion coreutils");
            exit(-1);
        }
    };

    if let Err(err) = result {
        eprintln!("{err}");
        exit(-1);
    }
}

pub fn cmd() -> Result<()> {
    println!(
        "hyperion {} - {}",
        env!("CARGO_PKG_NAME"),
        env!("CARGO_PKG_VERSION")
    );

    Ok(())
}

fn read_file_map(buf: &mut String, p: &str) -> Result<BTreeMap<Box<str>, Box<str>>> {
    let mut map = BTreeMap::new();

    let file = File::open(p).map_err(|err| anyhow!("{err}"))?;
    let mut file = BufReader::new(file);

    loop {
        buf.clear();
        let n = file.read_line(buf).map_err(|err| anyhow!("{err}"))?;
        if n == 0 {
            break;
        }
        let line = &buf[..n];

        let (key, val) = line
            .split_once(':')
            .expect("invalid /proc/<pid>/status format");
        let (key, val) = (key.trim(), val.trim());

        map.insert(key.into(), val.into());
    }

    Ok(map)
}
