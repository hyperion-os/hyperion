#![no_std]
#![no_main]
#![feature(format_args_nl)]

//

mod cat;

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
        "cat" => cat::cat(args),
        _ => coreutils(),
    };

    if let Err(err) = result {
        eprintln!("{err}");
        exit(-1);
    }
}

pub fn coreutils() -> Result<()> {
    println!(
        "hyperion {} - {}",
        env!("CARGO_PKG_NAME"),
        env!("CARGO_PKG_VERSION")
    );

    Ok(())
}
