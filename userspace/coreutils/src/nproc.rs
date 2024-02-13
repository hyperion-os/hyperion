use alloc::string::String;

use anyhow::{anyhow, Result};
use libstd::{fs::File, io::BufReader, println};

//

pub fn cmd<'a>(_: impl Iterator<Item = &'a str>) -> Result<()> {
    let cpuinfo = File::open("/proc/cpuinfo").map_err(|err| anyhow!("{err}"))?;
    let mut cpuinfo = BufReader::new(cpuinfo);

    let mut buf = String::new();

    let mut count = 0usize;
    loop {
        buf.clear();
        let n = cpuinfo
            .read_line(&mut buf)
            .map_err(|err| anyhow!("{err}"))?;
        if n == 0 {
            break;
        }
        let line = &buf[..n];

        if line.starts_with("processor") {
            count += 1;
        }
    }

    println!("{count}");

    Ok(())
}
