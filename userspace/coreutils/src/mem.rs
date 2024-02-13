use alloc::string::String;

use anyhow::{anyhow, Result};
use hyperion_num_postfix::NumberPostfix;
use libstd::{fs::File, io::BufReader, println};

//

pub fn cmd<'a>(_: impl Iterator<Item = &'a str>) -> Result<()> {
    let meminfo: File = File::open("/proc/meminfo").map_err(|err| anyhow!("{err}"))?;
    let mut meminfo = BufReader::new(meminfo);

    let mut total = None;
    let mut free = None;

    let mut buf = String::new();
    loop {
        buf.clear();
        let n = meminfo
            .read_line(&mut buf)
            .map_err(|err| anyhow!("{err}"))?;
        if n == 0 {
            break;
        }
        let line = buf.trim();
        if line.is_empty() {
            continue;
        }

        let (item, value) = line.split_once(':').unwrap();
        let value = value.trim();
        let (value, kb) = value
            .split_once(' ')
            .map(|(num, kb)| (num, Some(kb)))
            .unwrap_or((value, None));

        let mut num = value.parse::<usize>().map_err(|err| anyhow!("{err}"))?;
        if kb == Some("kb") {
            num *= 0x400;
        }

        match item {
            "MemTotal" => total = Some(num),
            "MemFree" => free = Some(num),
            _ => {}
        }
    }

    let total = total.unwrap();
    let used = total - free.unwrap();

    let p = used as f64 / total as f64 * 100.0;
    let used = used.postfix_binary();
    let total = total.postfix_binary();

    println!("Mem:\n - total: {total}B\n - used: {used}B ({p:3.1}%)");

    Ok(())
}
