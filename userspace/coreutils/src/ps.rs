use alloc::{format, string::String};
use core::ops::Deref;

use anyhow::{anyhow, Result};
use libstd::{eprintln, fs::Dir, println};

//

pub fn cmd<'a>(_: impl Iterator<Item = &'a str>) -> Result<()> {
    println!("{: >6} {: >7} {: >9} CMD", "PID", "THREADS", "TIME");

    let mut buf = String::new();

    for proc in Dir::open("/proc")
        .map_err(|err| anyhow!("{err}"))?
        .into_iter()
        // filter out non-PID entries
        .filter(|ent| ent.file_name.parse::<usize>().is_ok())
    {
        let contents =
            match super::read_file_map(&mut buf, &format!("/proc/{}/status", proc.file_name)) {
                Ok(v) => v,
                Err(e) => {
                    eprintln!("{e}");
                    continue;
                }
            };

        let pid = proc.file_name;
        let name = contents.get("Name").unwrap().deref();
        let threads = contents.get("Threads").unwrap().parse::<usize>().unwrap();
        let nanos = contents.get("Nanos").unwrap().parse::<usize>().unwrap();

        let time = time::Duration::nanoseconds(nanos as _);
        // let time_h = time.whole_hours();
        let time_m = time.whole_minutes() % 60;
        let time_s = time.whole_seconds() % 60;
        let time_ms = time.whole_milliseconds() % 1000;

        println!("{pid: >6} {threads: >7} {time_m: >2}:{time_s:02}.{time_ms:03} {name}");
    }

    Ok(())
}
