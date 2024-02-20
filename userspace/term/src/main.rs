use core::ffi;
use std::{
    io::{Read, Write},
    process::{exit, Command, Stdio},
    sync::{Arc, Mutex},
    thread,
};

use hyperion_windowing::{client::Connection, shared::Event};
use term::Term;

//

// mod fbo;
mod font;
mod term;

//

#[no_mangle]
extern "C" fn truncf(x: ffi::c_float) -> ffi::c_float {
    libm::truncf(x)
}

#[no_mangle]
extern "C" fn roundf(x: ffi::c_float) -> ffi::c_float {
    libm::roundf(x)
}

#[no_mangle]
extern "C" fn powf(x: ffi::c_float, y: ffi::c_float) -> ffi::c_float {
    libm::powf(x, y)
}

#[no_mangle]
extern "C" fn exp2f(x: ffi::c_float) -> ffi::c_float {
    libm::exp2f(x)
}

#[no_mangle]
extern "C" fn ceil(x: ffi::c_double) -> ffi::c_double {
    libm::ceil(x)
}

//

fn main() {
    let font = font::load_monospace_ttf();

    let wm = Connection::new().unwrap();
    let window = Box::leak(Box::new(wm.new_window().unwrap()));

    let term = Term::new(window.as_region(), font);

    let mut shell = Command::new("/bin/sh")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .arg("/")
        .spawn()
        .unwrap();

    let mut stdin = shell.stdin.take().unwrap();
    let mut stdout = shell.stdout.take().unwrap();
    let mut stderr = shell.stderr.take().unwrap();

    thread::spawn(move || {
        while let Ok(ev) = wm.next_event() {
            match ev {
                // TODO: send LEFT,RIGHT,UP,DOWN and others
                // Event::Keyboard {
                //     code: 88 | 101, // up or left
                //     state: ElementState::Pressed,
                // } => i = i.wrapping_add(1),
                // Event::Keyboard {
                //     code: 102 | 103, // down or right
                //     state: ElementState::Pressed,
                // } => i = i.wrapping_sub(1),
                Event::Text { ch } => {
                    let mut buf = [0u8; 4];
                    let str = ch.encode_utf8(&mut buf);
                    stdin.write_all(str.as_bytes()).unwrap();
                }
                _ => {}
            }
        }
    });

    let term = Arc::new(Mutex::new(term));
    let term2 = term.clone();

    thread::spawn(move || {
        let mut buf = [0u8; 512];
        loop {
            let n = stdout.read(&mut buf).unwrap();
            if n == 0 {
                return;
            }
            let data = &buf[..n];

            let mut t = term.lock().unwrap();
            t.write_bytes(data);
            t.flush();
        }
    });

    thread::spawn(move || {
        let mut buf = [0u8; 512];
        loop {
            let n = stderr.read(&mut buf).unwrap();
            if n == 0 {
                return;
            }
            let data = &buf[..n];

            let mut t = term2.lock().unwrap();
            t.write_bytes(data);
            t.flush();
        }
    });

    let ec = shell.wait().unwrap().code().unwrap_or(0);
    exit(ec);
}
