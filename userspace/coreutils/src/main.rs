#![no_std]
#![no_main]
#![feature(format_args_nl)]

//

extern crate alloc;

mod cat;
mod ls;
mod sleep;
mod touch;

//

use anyhow::Result;
use libstd::{eprintln, println, sys::exit, CliArgs};

//

#[no_mangle]
fn main(args: CliArgs) {
    let mut args = args.iter();
    let cmd = args.next().expect("arg 0 should always be there");

    let cmd = cmd.rsplit_once('/').map(|(_, rhs)| rhs).unwrap_or(cmd);

    let result = match cmd {
        "cat" => cat::cmd(args),
        "ls" => ls::cmd(args),
        "touch" => touch::cmd(args),
        "sleep" => sleep::cmd(args),
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
