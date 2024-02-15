use std::{
    collections::HashMap,
    fs::OpenOptions,
    io::{stdin, Read},
    path::Path,
};

//

fn main() {
    let stdin = stdin();

    // let mut buf = [0u8; 0];
    // let n = stdin.read(&mut buf).unwrap();
    // let line = std::str::from_utf8(&buf[..n]);
    // println!("{line:?}");

    // let mut buf = [0u8; 0];
    // let n = stdin.read(&mut buf).unwrap();
    // let line = std::str::from_utf8(&buf[..n]);
    // println!("{line:?}");

    // let mut buf = Vec::new();
    // let n = stdin.read_to_end(&mut buf).unwrap();
    // let line = std::str::from_utf8(&buf[..n]);
    // println!("{line:?}");

    // for line in stdin.lines() {
    //     println!("{line:?}");
    // }

    let mut buf = String::new();
    let n = stdin.read_line(&mut buf).unwrap();
    let line = &buf[..n];
    println!("{line:?}");

    // tracing_subscriber::fmt::init();
    // tracing::error!("test error");

    let path = Path::new("splash");
    let mut file = OpenOptions::new().read(true).open(path).unwrap();
    let mut buf = String::new();
    file.read_to_string(&mut buf).unwrap();

    let mut map = HashMap::<i32, i32>::new();
    map.insert(53, 35);
    map.insert(0, 1);

    println!("{map:?}");

    println!("{buf}");
}
