#![no_std]

//

extern crate alloc;

mod cat;
mod cp;
mod date;
mod echo;
mod ls;
mod mem;
mod nproc;
mod random;
mod sleep;
mod touch;

//

use anyhow::Result;
use libstd::{env, eprintln, println, sys::exit};

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
        "ls" => ls::cmd(args),
        "mem" => mem::cmd(args),
        "nproc" => nproc::cmd(args),
        "random" => random::cmd(args),
        "sleep" => sleep::cmd(args),
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
