use alloc::{format, string::String};

use anyhow::{anyhow, Result};
use libstd::{
    fs::{Dir, File},
    io::Read,
    println,
};

//

pub fn cmd<'a>(_: impl Iterator<Item = &'a str>) -> Result<()> {
    println!("{: >6} {: >7} {: >9} CMD", "PID", "THREADS", "TIME");

    let mut buf = String::new();

    for proc in Dir::open("/proc")
        .map_err(|err| anyhow!("{err}"))?
        // filter out non-PID entries
        .filter(|ent| ent.file_name.parse::<usize>().is_ok())
    {
        let contents = super::read_file_map(&mut buf, &format!("/proc/{}/status", proc.file_name))
            .map_err(|err| anyhow!("{err}"))?;

        let mut cmdline_file = File::open(&format!("/proc/{}/cmdline", proc.file_name))
            .map_err(|err| anyhow!("{err}"))?;
        let mut cmdline_buf = [255u8; 32];
        let n = cmdline_file
            .read(&mut cmdline_buf)
            .map_err(|err| anyhow!("{err}"))?;
        for b in cmdline_buf.iter_mut().filter(|s| **s == 0) {
            // null bytes are cli arg separators
            *b = b' ';
        }
        let cmdline = core::str::from_utf8(&cmdline_buf[..n]).map_err(|err| anyhow!("{err}"))?;

        let pid = proc.file_name;
        let threads = contents.get("Threads").unwrap().parse::<usize>().unwrap();
        let nanos = contents.get("Nanos").unwrap().parse::<usize>().unwrap();

        let time = time::Duration::nanoseconds(nanos as _);
        // let time_h = time.whole_hours();
        let time_m = time.whole_minutes() % 60;
        let time_s = time.whole_seconds() % 60;
        let time_ms = time.whole_milliseconds() % 1000;

        println!("{pid: >6} {threads: >7} {time_m: >2}:{time_s:02}.{time_ms:03} {cmdline}");
    }

    Ok(())
}
