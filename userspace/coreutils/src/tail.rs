use alloc::vec::Vec;

use anyhow::{anyhow, Result};
use libstd::println;

//

pub fn cmd<'a>(args: impl Iterator<Item = &'a str>) -> Result<()> {
    let args: Vec<&str> = args.collect();

    if !args.contains(&"-f") {
        return Err(anyhow!("not yet implemented, -f flag is required"));
    }

    let Some(pid) = args
        .iter()
        .find_map(|s| s.strip_prefix("--pid="))
        .or_else(|| {
            args.iter()
                .position(|f| *f == "--pid")
                .and_then(|pid_idx| args.get(pid_idx))
                .copied()
        })
    else {
        return Err(anyhow!("not yet implemented, --pid=PID flag is required"));
    };

    let Ok(pid) = pid.parse::<usize>() else {
        return Err(anyhow!("PID should be a number"));
    };

    let code = libstd::sys::waitpid(pid);

    println!("exit code {code}");
    Ok(())
}
