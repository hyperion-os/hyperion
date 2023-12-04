#![no_std]
#![no_main]
#![feature(format_args_nl)]

//

use core::{
    fmt::{self, Display},
    str::from_utf8,
};

use libstd::{
    eprintln,
    fs::{File, STDOUT},
    io::Write,
    print, println,
    sys::exit,
    CliArgs,
};

//

#[no_mangle]
fn main(args: CliArgs) {
    let mut args = args.iter();
    let a0 = args.next().expect("arg 0 should always be there");

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

    let mut buf = [0u8; 2];
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
            write!(f, "{}", *byte as char)?;
            // write!(f, "{byte:x} ")?;
        }
        Ok(())
    }
}
