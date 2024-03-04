use std::{
    env::args,
    fs::File,
    io::{stdin, stdout, BufRead, BufReader, Read, Write},
    process::{Command, Stdio},
    str,
};

use hyperion_escape::encode::*;
use hyperion_syscall::exit;

//

fn main() {
    let name = args().next().unwrap();
    let name = name.rsplit('/').next().unwrap();

    if let Some(file) = args().nth(1) {
        if file.as_str() == "-c" {
            immediate();
        } else {
            script(file)
        }
    } else {
        interactive(name)
    }
}

fn run_line(line: &str) {
    let line = line.trim();

    if line.starts_with('#') {
        return;
    }

    let mut parts = line.split(' ').map(|s| s.trim()).filter(|s| !s.is_empty());

    let Some(cmd) = parts.next() else {
        return;
    };

    match cmd {
        "exit" => exit(0),
        "" => return,
        _ => {}
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
}

fn immediate() {
    let cmd = args().skip(2).fold(String::new(), |mut acc, s| {
        acc.push_str(s.as_str());
        acc.push_str(" ");
        acc
    });
    run_line(&cmd);
}

fn script(file: String) {
    let mut script = BufReader::new(File::open(file).unwrap());
    let mut line = String::new();

    while let Ok(n) = script.read_line(&mut line) {
        if n == 0 {
            break;
        }

        run_line(&line[..n]);
        line.clear();
    }
}

fn interactive(name: &str) {
    let mut stdin = stdin().lock();
    // let mut history = Vec::new();
    // let mut browsing_idx = None;

    prompt(name);

    let mut line = String::new();
    let mut buf = [0u8; 1];
    loop {
        let n = stdin.read(&mut buf).unwrap();
        let input = str::from_utf8(&buf[..n]).unwrap();

        // handle backspace
        if buf[..n] == [8] {
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
            run_line(&line[..n]);

            // clear the input buffer and start the next cmdline
            line.clear();
            prompt(name);
        }
    }
}

fn prompt(name: &str) {
    print!("[{name}]# ");
    stdout().flush().unwrap();
}
