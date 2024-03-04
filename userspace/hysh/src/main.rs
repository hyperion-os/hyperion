use std::{
    env::args,
    fs::File,
    io::{stdin, stdout, BufRead, BufReader, Read, StdinLock, Write},
    iter,
    process::{Command, Stdio},
    slice, str,
};

use hyperion_escape::{
    decode::{DecodedPart, EscapeDecoder},
    encode::*,
};
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
    Shell::new(name).run();
}

#[derive(Debug)]
struct Shell<'a> {
    name: &'a str,
    cmdline: String,
    cursor: usize,

    history: Vec<String>,
    history_selection: usize,
}

impl<'a> Shell<'a> {
    fn new(name: &'a str) -> Self {
        Self {
            name,
            cmdline: String::new(),
            cursor: 0,

            history: Vec::new(),
            history_selection: 0,
        }
    }

    fn run(&mut self) {
        self.prompt();

        let mut escape_decoder = EscapeDecoder::new();
        let mut stdin = stdin().lock();
        let mut buf = [0u8; 1];

        for part in iter::from_fn(move || {
            let n = stdin.read(&mut buf).unwrap();
            if n == 0 {
                return None;
            }

            Some(buf[0])
        })
        .map(move |byte| escape_decoder.next(byte))
        {
            // println!("{part:?}");
            match part {
                DecodedPart::Byte(b) => self.add_byte(b),
                DecodedPart::Bytes(bytes) => {
                    for b in bytes.into_iter().take_while(|b| *b != 0) {
                        self.add_byte(b)
                    }
                }
                DecodedPart::CursorUp(_) => self.up(),
                DecodedPart::CursorDown(_) => self.down(),
                DecodedPart::CursorLeft(_) => self.left(),
                DecodedPart::CursorRight(_) => self.right(),
                _ => {}
            }
        }
    }

    fn left(&mut self) {
        if self.cursor == 0 {
            return;
        }

        self.cursor -= 1;
        print!("{}", CursorLeft(1));
        stdout().flush().unwrap();
    }

    fn right(&mut self) {
        if self.cursor >= self.cmdline.len() {
            return;
        }

        self.cursor += 1;
        print!("{}", CursorRight(1));
        stdout().flush().unwrap();
    }

    fn up(&mut self) {
        if self.history_selection == 0 {
            return;
        }
        self.select_history(self.history_selection - 1);
    }

    fn down(&mut self) {
        if self.history_selection == self.history.len() {
            return;
        }
        self.select_history(self.history_selection + 1);
    }

    fn select_history(&mut self, n: usize) {
        self.history_selection = n;

        // reset the cursor
        for _ in 0..self.cmdline.len() {
            print!("{} {}", CursorLeft(1), CursorLeft(1));
        }
        self.cursor = 0;

        // set the cmdline to one from the history buffer
        self.cmdline.clear(); // don't assign to keep the internal String buffer
        if let Some(history) = self.history.get(self.history_selection) {
            // history selection can go up to the length of the history array, which means that no old commands are selected
            self.cmdline.push_str(history.as_str());
        }

        // move the cursor to the end
        print!("{}", self.cmdline);
        self.cursor = self.cmdline.len();
        stdout().flush().unwrap();
    }

    fn enter(&mut self) {
        println!();

        // if the cmdline had just whitespaces: clear extra whitespaces but don't append to history
        if !self.cmdline.trim().is_empty() {
            run_line(self.cmdline.as_str());

            self.history.push(self.cmdline.clone());
            self.history_selection += 1;
        }

        // clear the input buffer and start the next cmdline
        self.cmdline.clear();
        self.cursor = 0;
        self.prompt();
    }

    fn backspace(&mut self) {
        // println!("{self:?}");
        if self.cursor == 0 {
            return;
        }
        self.left();
        self.cmdline.remove(self.cursor);

        print!("{} ", &self.cmdline[self.cursor..]);
        for _ in 0..self.cmdline[self.cursor..].len() + 1 {
            print!("{}", CursorLeft(1));
        }

        stdout().flush().unwrap();
    }

    fn delete(&mut self) {
        if self.cursor >= self.cmdline.len() {
            return;
        }

        self.right();
        self.backspace();
    }

    fn tab(&mut self) {}

    fn add_byte(&mut self, b: u8) {
        match b {
            8 => self.backspace(),
            127 => self.delete(),
            b'\n' => self.enter(),
            b'\t' => self.tab(),
            _ => {
                if let Some(ch) = char::from_u32(b as _) {
                    // print the pressed char
                    self.cmdline.insert(self.cursor, ch);
                    print!("{} ", &self.cmdline[self.cursor..]);
                    for _ in 0..self.cmdline[self.cursor..].len() {
                        print!("{}", CursorLeft(1));
                    }
                    stdout().flush().unwrap();
                    self.cursor += 1;
                }
            }
        }
    }

    fn prompt(&self) {
        print!("[{}]# ", self.name);
        stdout().flush().unwrap();
    }
}
