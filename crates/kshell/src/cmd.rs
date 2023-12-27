use alloc::{string::String, sync::Arc, vec::Vec};
use core::any::Any;

use hyperion_kernel_impl::FileDescriptor;
use hyperion_scheduler::{
    lock::{Futex, Mutex},
    process, schedule,
};
use hyperion_syscall::fs::FileDesc;
use hyperion_vfs::{
    device::FileDevice,
    error::{IoError, IoResult},
    tree::FileRef,
};

//

pub struct Command {
    program: String,
    args: Vec<String>,

    stdin: Option<Arc<dyn FileDescriptor>>,
    stdout: Option<Arc<dyn FileDescriptor>>,
    stderr: Option<Arc<dyn FileDescriptor>>,
}

impl Command {
    pub fn new(program: impl Into<String>) -> Self {
        Self {
            program: program.into(),
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

    pub fn spawn(&mut self) {
        // let mut c = std::process::Command::new("ls");
        // c.stdin(output)
        // c.output();
        // c.spawn();

        // struct KernelLogs;

        // impl FileDevice for KernelLogs {
        //     fn as_any(&self) -> &dyn Any {
        //         self
        //     }

        //     fn len(&self) -> usize {
        //         0
        //     }

        //     fn set_len(&mut self, _: usize) -> IoResult<()> {
        //         Err(IoError::PermissionDenied)
        //     }

        //     fn read(&self, _: usize, _: &mut [u8]) -> IoResult<usize> {
        //         Err(IoError::PermissionDenied)
        //     }

        //     fn write(&mut self, _: usize, buf: &[u8]) -> IoResult<usize> {
        //         if let Ok(str) = core::str::from_utf8(buf) {
        //             hyperion_log::println!("{str}");
        //         }

        //         Ok(buf.len())
        //     }
        // }

        // let program = self.program.clone();

        // let stdin = self.stdin.clone();
        // let stdout = self.stdout.clone();
        // let stderr = self
        //     .stderr
        //     .clone()
        //     .unwrap_or_else(|| Arc::new(Mutex::new(KernelLogs)));

        // process();

        // schedule(move || {
        //     // set its name
        //     hyperion_scheduler::rename(program.as_str());

        //     // setup the STDIO
        //     if let Some(stdin) = stdin {
        //         hyperion_kernel_impl::fd_replace(FileDesc(0), stdin);
        //     } else {
        //         hyperion_kernel_impl;
        //     }
        //     if let Some(stdout) = stdout {
        //         hyperion_kernel_impl::set_fd(FileDesc(1), stdout);
        //     }
        //     hyperion_kernel_impl::set_fd(FileDesc(2), stderr);

        //     // load and exec the binary
        //     let args: Vec<&str> = [name] // TODO: actually load binaries from vfs
        //         .into_iter()
        //         .chain(args.as_deref().iter().flat_map(|args| args.split(' ')))
        //         .collect();
        //     let args = &args[..];

        //     hyperion_log::trace!("spawning \"{name}\" with args {args:?}");

        //     let loader = hyperion_loader::Loader::new(self.as_ref());

        //     loader.load();

        //     if loader.enter_userland(args).is_none() {
        //         hyperion_log::debug!("entry point missing");
        //     }
        // });
    }
}
