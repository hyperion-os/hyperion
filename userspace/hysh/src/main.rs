use std::{
    env::args,
    io::{stdin, stdout, Read, Write},
    process::{Command, Stdio},
    str,
};

use hyperion_escape::encode::*;
use hyperion_syscall::exit;

//

fn main() {
    let name = args().next().unwrap();

    let mut stdin = stdin().lock();

    prompt(&name);

    let mut line = String::new();
    let mut buf = [0u8; 1];
    loop {
        let n = stdin.read(&mut buf).unwrap();
        let input = str::from_utf8(&buf[..n]).unwrap();

        // handle backspace
        if &buf[..n] == &[8] {
            if line.pop().is_some() {
                print!("{} {}", CursorLeft(1), CursorLeft(1));
                stdout().flush().unwrap();
            }
            continue;
        }

        // print the pressed char
        print!("{input}");
        stdout().flush().unwrap();
        line.push_str(input);

        // TODO: save the characters after \n
        if let Some(n) = line.find('\n') {
            // run the commandline
            let cmdline = &line[..n];
            let mut parts = cmdline.split(' ').filter(|s| !s.is_empty());

            let Some(cmd) = parts.next() else {
                line.clear();
                prompt(&name);
                continue;
            };

            if cmd == "exit" {
                exit(0);
            }

            let cli = format!("/bin/{cmd}");

            let mut cmd = Command::new(cli)
                .stdin(Stdio::inherit())
                .stdout(Stdio::inherit())
                .stderr(Stdio::inherit())
                .args(parts)
                .spawn()
                .unwrap();

            cmd.wait().unwrap();

            println!();

            line.clear();
            prompt(&name);
        }
    }
}

fn prompt(name: &str) {
    print!("[{name}]# ");
    stdout().flush().unwrap();
}
