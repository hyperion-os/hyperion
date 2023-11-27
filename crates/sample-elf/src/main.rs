#![no_std]
#![no_main]
#![feature(format_args_nl, slice_internals)]

//

use libstd::{
    alloc::{format, string::String},
    println,
    sys::{
        err::Result,
        net::{Protocol, SocketDomain, SocketType},
        *,
    },
    thread::spawn,
    CliArgs,
};

//

mod io; // partial std::io

//

fn run_server() -> Result<()> {
    let server = socket(SocketDomain::LOCAL, SocketType::STREAM, Protocol::LOCAL)?;
    bind(server, "/dev/server.sock")?;

    rename("local server")?;

    let mut i = 0usize;
    loop {
        let conn = accept(server)?;
        println!("connected");

        let msg = format!("Hello {i}");
        i += 1;

        spawn(move || {
            send(conn, msg.as_bytes(), 0).unwrap();

            let mut buf = [0u8; 64];
            let len = recv(conn, &mut buf, 0).unwrap();
            assert_eq!(&buf[..len], b"ack");

            println!("server done");
        });
    }
}

fn run_client() -> Result<()> {
    let client = socket(SocketDomain::LOCAL, SocketType::STREAM, Protocol::LOCAL)?;
    connect(client, "/dev/server.sock")?;

    rename("local client")?;

    println!("connected");

    loop {
        let mut buf = [0u8; 64];
        let len = recv(client, &mut buf, 0)?;

        println!("got `{:?}`", core::str::from_utf8(&buf[..len]));

        if buf[..len].ends_with(b"2") {
            panic!()
        }

        send(client, b"ack", 0)?;
    }
}

#[no_mangle]
pub fn main(_args: CliArgs) {
    loop {
        String::leak("test".into());
    }

    if run_server().is_err() {
        run_client().unwrap();
    }

    /* match args.iter().next().expect("arg0 to be present") {
        // busybox style single binary 'coreutils'
        "/bin/run" => {
            let inc = Arc::new(AtomicUsize::new(0));

            for _n in 0..80 {
                let inc = inc.clone();
                spawn(move || {
                    // println!("hello from thread {_n}");
                    inc.fetch_add(1, Ordering::Relaxed);
                });
            }

            let hpet = File::open("/dev/hpet").expect("failed to open /dev/hpet");
            let mut buf = [0u8; 256];
            let bytes = hpet.read(&mut buf).expect("failed to read from a file");

            println!("/dev/hpet bytes: {:?}", &buf[..bytes]);
            drop(hpet);

            for i in 0..10 {
                let file = OpenOptions::new()
                    .write(true)
                    .create(true)
                    .open(&format!("/tmp/tmp-{i}"))
                    .expect("failed to open /testfile");
                file.write(b"testing data").expect("failed to write");
                drop(file);
            }

            let mut next = timestamp().unwrap() as u64;
            for i in next / 1_000_000_000.. {
                println!("inc at: {}", inc.load(Ordering::Relaxed));

                nanosleep_until(next);
                next += 1_000_000_000;

                println!("seconds since boot: {i}");
            }
        }

        "/bin/task1" => {
            rename("<Get_Input>").unwrap();

            let pid: usize = args
                .iter()
                .nth(1)
                .expect("missing arg: PID")
                .parse()
                .expect("failed to parse PID");

            let mut line = String::new();
            loop {
                line.clear();
                let mut input_channel = BufReader::new(SimpleIpcInputChannel);
                input_channel.read_line(&mut line).unwrap();

                let input = line.trim();
                println!("<Get_Input>: '{input}'");
                send(pid, input.as_bytes()).unwrap();
                send(pid, b"\n").unwrap(); // BufReader::read_line waits for a \n
            }
        }

        "/bin/task2" => {
            rename("<Clean_Input>").unwrap();

            let pid: usize = args
                .iter()
                .nth(1)
                .expect("missing arg: PID")
                .parse()
                .expect("failed to parse PID");

            let mut line = String::new();
            loop {
                line.clear();
                let mut input_channel = BufReader::new(SimpleIpcInputChannel);
                input_channel.read_line(&mut line).unwrap();

                let messy_string = line.trim();
                let clean_string = messy_string.replace(|c| !char::is_alphabetic(c), "");
                println!("<Clean_Input>: '{clean_string}'");

                send(pid, clean_string.as_bytes()).unwrap();
                send(pid, b"\n").unwrap(); // BufReader::read_line waits for a \n
            }
        }

        "/bin/task3" => {
            rename("<Find_Missing>").unwrap();

            let mut line = String::new();

            loop {
                line.clear();
                let mut input_channel = BufReader::new(SimpleIpcInputChannel);
                input_channel.read_line(&mut line).unwrap();

                println!("got '{}'", line.trim());

                let mut found = [false; 26];
                for c in line.trim().chars() {
                    found[((c as u8).to_ascii_lowercase() - b'a') as usize] = true;
                }

                let mut buf = String::new();
                for missing in found
                    .iter()
                    .enumerate()
                    .filter(|(_, found)| !*found)
                    .map(|(i, _)| i)
                {
                    buf.push((missing as u8 + b'a') as char);
                }
                println!("<Find_Missing>: '{buf}'");

                // PID 1 is known to be kshell, for now
                // send(1, buf.as_bytes());
            }
        }

        tool => panic!("unknown tool {tool}"),
    } */
}
