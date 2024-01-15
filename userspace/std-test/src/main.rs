use std::{collections::HashMap, fs::OpenOptions, io::Read, path::Path};

//

fn main() {
    tracing_subscriber::fmt()
        .with_max_level(tracing::level_filters::LevelFilter::TRACE)
        .init();

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
