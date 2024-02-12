use alloc::{boxed::Box, format, string::String, sync::Arc};
use core::{
    any::Any,
    fmt::{self, Display, Write},
};

use hyperion_scheduler::lock::{Futex, Lazy, Once};
use hyperion_vfs::{
    device::{DirectoryDevice, FileDevice},
    error::{IoError, IoResult},
    ramdisk::{File, StaticRoFile},
    tree::{IntoNode, Node},
    AnyMutex,
};
use lock_api::{Mutex, RawMutex};

//

pub fn init(root: impl IntoNode) {
    root.into_node().mount("proc", ProcFs::new());

    // let root = root.into_node().find("/proc", true).unwrap();

    // root.install_dev("meminfo", MemInfo);
}

//

pub struct MemInfo {
    total: usize,
    free: usize,
}

impl fmt::Display for MemInfo {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        writeln!(f, "MemTotal:{}", self.total)?;
        writeln!(f, "MemFree:{}", self.free)?;
        Ok(())
    }
}

//

pub struct Cmdline;

impl FileDevice for Cmdline {
    fn driver(&self) -> &'static str {
        "procfs"
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn len(&self) -> usize {
        hyperion_boot::args::get().cmdline.len()
    }

    fn set_len(&mut self, _: usize) -> IoResult<()> {
        Err(IoError::PermissionDenied)
    }

    fn read(&self, offset: usize, buf: &mut [u8]) -> IoResult<usize> {
        hyperion_boot::args::get()
            .cmdline
            .as_bytes()
            .read(offset, buf)
    }

    fn write(&mut self, _: usize, _: &[u8]) -> IoResult<usize> {
        Err(IoError::PermissionDenied)
    }
}

//

pub struct ProcFs<Mut> {
    cmdline: Once<Node<Mut>>,
    version: Once<Node<Mut>>,
}

impl<Mut> ProcFs<Mut> {
    pub const fn new() -> Self {
        Self {
            cmdline: Once::new(),
            version: Once::new(),
        }
    }
}

impl<Mut: AnyMutex> DirectoryDevice<Mut> for ProcFs<Mut> {
    fn driver(&self) -> &'static str {
        "procfs"
    }

    fn get_node(&mut self, name: &str) -> IoResult<Node<Mut>> {
        match name {
            "meminfo" => {
                let pfa = &*hyperion_mem::pmm::PFA;

                // create a snapshot of the system memory info to fix some data races
                Ok(Node::new_file(DisplayFile(MemInfo {
                    total: pfa.usable_mem() / 0x400,
                    free: pfa.free_mem() / 0x400,
                })))
            }
            "cmdline" => Ok(self
                .cmdline
                .call_once(|| Node::new_file(DisplayFile(hyperion_boot::args::get().cmdline)))
                .clone()),
            "version" => Ok(self
                .version
                .call_once(|| {
                    Node::new_file(DisplayFile(format!(
                        "{} version {} #{} {}",
                        hyperion_kernel_info::NAME,
                        hyperion_kernel_info::VERSION,
                        hyperion_kernel_info::BUILD_REV,
                        hyperion_kernel_info::BUILD_TIME,
                    )))
                })
                .clone()),
            _ => Err(IoError::NotFound),
        }
    }

    fn create_node(&mut self, name: &str, node: Node<Mut>) -> IoResult<()> {
        Err(IoError::PermissionDenied)
    }

    fn nodes(&mut self) -> IoResult<Arc<[Arc<str>]>> {
        static FILES: Lazy<Arc<[Arc<str>]>> =
            Lazy::new(|| ["meminfo".into(), "cmdline".into(), "version".into()].into());

        Ok(FILES.clone())
    }
}

//

struct DisplayFile<T>(T);

impl<T: Display + Send + Sync + 'static> FileDevice for DisplayFile<T> {
    fn driver(&self) -> &'static str {
        "procfs"
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn len(&self) -> usize {
        let mut l = FmtLength { len: 0 };
        write!(&mut l, "{}", self.0).unwrap();
        l.len
    }

    fn set_len(&mut self, _: usize) -> IoResult<()> {
        Err(IoError::PermissionDenied)
    }

    fn read(&self, offset: usize, buf: &mut [u8]) -> IoResult<usize> {
        let mut w = FmtOffsetBuf {
            offset,
            buf,
            cursor: 0,
        };
        write!(&mut w, "{}", self.0).unwrap();
        Ok(w.cursor.saturating_sub(w.offset).min(w.buf.len()))
    }

    fn write(&mut self, _: usize, _: &[u8]) -> IoResult<usize> {
        Err(IoError::PermissionDenied)
    }
}

//

struct FmtLength {
    len: usize,
}

impl fmt::Write for FmtLength {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        self.len += s.len();
        Ok(())
    }
}

struct FmtOffsetBuf<'a> {
    // 0..offset | offset..offset+buf.len() | offset+buf.len()..
    //   ignored | written to buf           | ignored
    offset: usize,
    buf: &'a mut [u8],
    cursor: usize,
}

impl fmt::Write for FmtOffsetBuf<'_> {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        let s = s.as_bytes();

        // write s into buf when s starts from self.cursor and buf starts from offset
        // both have some virtual empty space before and after
        //
        // so it is like an unaligned slice copy
        //           +--------+
        //           |abc     |
        //           +--------+
        //             |
        //            \/
        //      +--------+
        //      |     abc|
        //      +--------+
        if let Some(s) = s.get(self.offset.saturating_sub(self.cursor)..) {
            let limit = s.len().min(self.buf.len());
            self.buf[..limit].copy_from_slice(&s[..limit]);
        }

        self.cursor += s.len();

        Ok(())
    }
}
