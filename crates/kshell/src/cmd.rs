use alloc::{boxed::Box, string::String, sync::Arc, vec::Vec};

use anyhow::{anyhow, Result};
use hyperion_futures::mpmc::Sender;
use hyperion_kernel_impl::{FileDescData, FileDescriptor, VFS_ROOT};
use hyperion_scheduler::lock::Lazy;

//

pub static NULL_DEV: Lazy<Arc<dyn FileDescriptor>> =
    Lazy::new(|| Arc::new(FileDescData::open("/dev/null").unwrap()));

pub static LOG_DEV: Lazy<Arc<dyn FileDescriptor>> =
    Lazy::new(|| Arc::new(FileDescData::open("/dev/log").unwrap()));

//

pub struct Command {
    program: String,
    args: Vec<String>,

    on_close: Option<Sender<()>>,

    stdin: Option<Arc<dyn FileDescriptor>>,
    stdout: Option<Arc<dyn FileDescriptor>>,
    stderr: Option<Arc<dyn FileDescriptor>>,
}

impl Command {
    pub fn new(program: impl Into<String>) -> Self {
        Self {
            program: program.into(),
            args: Vec::new(),

            on_close: None,

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

    pub fn on_close(&mut self, tx: Sender<()>) -> &mut Self {
        self.on_close = Some(tx);
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

    pub fn spawn(&mut self) -> Result<()> {
        let program = self.program.clone();
        let args = self.args.clone();
        let elf = Self::load_elf(&program)?;

        let on_close = self.on_close.clone();

        let stdin = self.stdin.clone().unwrap_or_else(|| NULL_DEV.clone());
        let stdout = self.stdout.clone().unwrap_or_else(|| NULL_DEV.clone());
        let stderr = self.stderr.clone().unwrap_or_else(|| LOG_DEV.clone());

        hyperion_kernel_impl::exec(
            program,
            args,
            stdin,
            stdout,
            stderr,
            on_close.map(|sender| Box::new(move || _ = sender.send(())) as _),
        );

        Ok(())
    }

    fn load_elf(path: &str) -> Result<Vec<u8>> {
        let mut elf = Vec::new();

        let bin = VFS_ROOT
            .find_file(path, false, false)
            .map_err(|err| anyhow!("unknown command `{path}`: {err}"))?;

        let bin = bin.lock_arc();

        loop {
            let mut buf = [0; 64];
            let len = bin.read(elf.len(), &mut buf).unwrap();
            elf.extend_from_slice(&buf[..len]);
            if len == 0 {
                break;
            }
        }

        Ok(elf)
    }
}
