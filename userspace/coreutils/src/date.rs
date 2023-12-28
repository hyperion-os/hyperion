use anyhow::{anyhow, Result};
use libstd::{fs::File, io::Read, println};
use time::OffsetDateTime;

//

pub fn cmd<'a>(_: impl Iterator<Item = &'a str>) -> Result<()> {
    let mut rtc = File::open("/dev/rtc")
        .map_err(|err| anyhow!("couldn't read the clock `/dev/rtc`: {err}"))?;

    let mut timestamp = [0u8; 8];
    let mut n = 0;
    rtc.read_exact(&mut timestamp, &mut n)?;
    assert_eq!(n, timestamp.len());

    let date = OffsetDateTime::from_unix_timestamp(i64::from_le_bytes(timestamp))
        .map_err(|err| anyhow!("invalid timestamp: {err}"))?;

    println!("{date}");

    Ok(())
}
