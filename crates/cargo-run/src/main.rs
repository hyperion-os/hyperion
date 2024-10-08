use std::{
    env::{current_dir, set_current_dir},
    process::{exit, Command},
};

use clap::Parser;

//

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// number of SMP CPUs
    #[arg(short, long, alias = "smp", value_name = "nproc", default_value = "4")]
    cpus: Option<usize>,

    /// amount of physical memory
    #[arg(short, long, value_name = "mem", value_parser = mem, default_value = "1g")]
    mem: Option<String>,

    /// enable KVM in QEMU
    #[arg(short, long, value_name = "enabled", default_value = "true")]
    kvm: Option<bool>,

    /// enable UEFI in QEMU
    #[arg(short, long, value_name = "enabled", default_value = "false")]
    uefi: Option<bool>,

    /// QEMU debug level
    #[arg(short, long, value_name = "enabled", default_value = "0")]
    debug: Option<u8>,

    /// build the kernel with optimizations
    #[arg(long)]
    release: bool,

    /// build the kernel with more optimizations
    #[arg(long)]
    release_lto: bool,

    /// run unit tests in QEMU
    #[arg(short, long)]
    test: bool,

    /// start QEMU with -s -S
    #[arg(short, long)]
    gdb: bool,
}

//

fn main() {
    let args = Args::parse();

    find_makefile();

    let mut cmd = Command::new("make");

    if args.test {
        cmd.arg("test");
    } else {
        cmd.arg("run");
    }

    if let Some(cpus) = args.cpus {
        cmd.arg(format!("CPUS={cpus}"));
        // cmd.env("CPUS", cpus);
    }

    if let Some(mem) = args.mem {
        cmd.arg(format!("MEMORY={mem}"));
    }

    if let Some(debug) = args.debug {
        cmd.arg(format!("DEBUG={debug}"));
    }

    if args.gdb {
        cmd.arg("GDB=true");
    }

    cmd.arg(format!("KVM={}", args.kvm.unwrap_or(true)));
    cmd.arg(format!("UEFI={}", args.uefi.unwrap_or(false)));
    if args.release && !args.release_lto {
        cmd.arg("PROFILE=release");
    }
    if args.release_lto {
        cmd.arg("PROFILE=release-lto");
    }

    cmd.spawn().unwrap().wait().unwrap();
}

fn mem(s: &str) -> Result<String, String> {
    let b = s.as_bytes();
    let Some((scale, num)) = b.split_last() else {
        return Err("empty str".into());
    };

    match *scale {
        b't' | b'g' | b'm' | b'k' | b'b' => {}
        s => return Err(format!("unknown scale '{}'", s as char)),
    }

    let _num = std::str::from_utf8(num)
        .map_err(|err| format!("{err}"))
        .and_then(|num| num.parse::<usize>().map_err(|err| format!("{err}")))?;

    Ok(s.into())
}

fn find_makefile() {
    let mut cd = current_dir().unwrap();

    loop {
        if cd.join("Makefile").try_exists().unwrap() {
            break;
        };

        if !cd.pop() {
            eprintln!("couldn't find Makefile");
            exit(-1);
        };
    }

    set_current_dir(cd).unwrap();
}
