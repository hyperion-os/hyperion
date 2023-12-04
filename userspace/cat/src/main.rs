#![no_std]
#![no_main]
#![feature(format_args_nl)]

//

use core::fmt::{self, Display};

use libstd::{eprintln, fs::File, print, println, sys::exit, CliArgs};

//

#[no_mangle]
fn main(args: CliArgs) {
    let mut args = args.iter();
    let _ = args.next().expect("arg 0 should always be there");

    let Some(a1) = args.next() else {
        eprintln!("expected at least one argument");
        exit(-1);
    };

    let file = match File::open(a1) {
        Ok(f) => f,
        Err(err) => {
            eprintln!("couldn't open `{a1}`: {err}");
            exit(-2);
        }
    };

    let mut buf = [0u8; 512];
    loop {
        let len = match file.read(&mut buf) {
            Ok(l) => l,
            Err(err) => {
                eprintln!("couldn't read `{a1}`: {err}");
                exit(-3);
            }
        };
        let buf = &buf[..len];

        if len == 0 {
            break;
        }

        print!("{}", Bytes(buf));
    }

    println!();
}

struct Bytes<'a>(&'a [u8]);

impl<'a> Display for Bytes<'a> {
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
