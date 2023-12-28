#![no_std]

//

extern crate alloc;

mod cat;
mod echo;
mod ls;
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
        "echo" => echo::cmd(args),
        "ls" => ls::cmd(args),
        "random" => random::cmd(args),
        "sleep" => sleep::cmd(args),
        "touch" => touch::cmd(args),
        _ => crate::cmd(),
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
