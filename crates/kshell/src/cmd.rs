use alloc::{borrow::Cow, string::String, sync::Arc, vec::Vec};
use core::{any::Any, str::from_utf8};

use hyperion_kernel_impl::{FileDescData, FileDescriptor, VFS_ROOT};
use hyperion_log::*;
use hyperion_scheduler::{lock::Lazy, schedule};
use hyperion_syscall::{err::Result, fs::FileDesc};

//

pub struct Command {
    program: String,
    program_elf: Cow<'static, [u8]>,
    args: Vec<String>,

    stdin: Option<Arc<dyn FileDescriptor>>,
    stdout: Option<Arc<dyn FileDescriptor>>,
    stderr: Option<Arc<dyn FileDescriptor>>,
}

impl Command {
    pub fn new(program: impl Into<String>, program_elf: Cow<'static, [u8]>) -> Self {
        Self {
            program: program.into(),
            program_elf,
            args: Vec::new(),

            stdin: None,
            stdout: None,
            stderr: None,
        }
    }

    pub fn arg(&mut self, arg: impl Into<String>) -> &mut Self {
        self.args.push(arg.into());
        self
    }

    pub fn args(&mut self, args: impl IntoIterator<Item = impl Into<String>>) -> &mut Self {
        self.args.extend(args.into_iter().map(|a| a.into()));
        self
    }

    pub fn stdin(&mut self, stdin: Arc<dyn FileDescriptor>) -> &mut Self {
        self.stdin = Some(stdin);
        self
    }

    pub fn stdout(&mut self, stdout: Arc<dyn FileDescriptor>) -> &mut Self {
        self.stdout = Some(stdout);
        self
    }

    pub fn stderr(&mut self, stderr: Arc<dyn FileDescriptor>) -> &mut Self {
        self.stderr = Some(stderr);
        self
    }

    pub fn spawn(&mut self) {
        // let mut c = std::process::Command::new("ls");
        // c.stdin(output)
        // c.output();
        // c.spawn();

        let program = self.program.clone();
        let elf = self.program_elf.clone();
        let args = self.args.clone();

        static NULL_DEV: Lazy<Arc<dyn FileDescriptor>> =
            Lazy::new(|| Arc::new(FileDescData::open("/dev/null").unwrap()));

        static LOG_DEV: Lazy<Arc<dyn FileDescriptor>> =
            Lazy::new(|| Arc::new(FileDescData::open("/dev/log").unwrap()));

        let stdin = self.stdin.clone().unwrap_or_else(|| NULL_DEV.clone());
        let stdout = self.stdout.clone().unwrap_or_else(|| NULL_DEV.clone());
        let stderr = self.stderr.clone().unwrap_or_else(|| LOG_DEV.clone());

        schedule(move || {
            // set its name
            hyperion_scheduler::rename(program.as_str());

            // setup the STDIO
            hyperion_kernel_impl::fd_replace(FileDesc(0), stdin);
            hyperion_kernel_impl::fd_replace(FileDesc(1), stdout);
            hyperion_kernel_impl::fd_replace(FileDesc(2), stderr);

            // load and exec the binary
            let args: Vec<&str> = [program.as_str()] // TODO: actually load binaries from vfs
                .into_iter()
                .chain(args.iter().flat_map(|args| args.split(' ')))
                .collect();
            let args = &args[..];

            trace!("spawning \"{program}\" with args {args:?}");

            let loader = hyperion_loader::Loader::new(elf.as_ref());

            loader.load();

            if loader.enter_userland(args).is_none() {
                hyperion_log::debug!("entry point missing");
            }
        });
    }
}
