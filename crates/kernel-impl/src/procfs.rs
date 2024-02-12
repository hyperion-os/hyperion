use alloc::{boxed::Box, string::String, sync::Arc};
use core::{any::Any, fmt::Write};

use hyperion_vfs::{
    device::{DirectoryDevice, FileDevice},
    error::{IoError, IoResult},
    ramdisk::File,
    tree::{IntoNode, Node},
};
use lock_api::{Mutex, RawMutex};

//

pub fn init(root: impl IntoNode) {
    root.into_node().mount("proc", ProcFs);

    // let root = root.into_node().find("/proc", true).unwrap();

    // root.install_dev("meminfo", MemInfo);
}

//

// pub struct MemInfo {
//     inner: Box<[u8]>,
//     // total: usize,
//     // free: usize,
// }

// impl FileDevice for MemInfo {
//     fn as_any(&self) -> &dyn Any {
//         self
//     }

//     fn len(&self) -> usize {
//         1
//     }

//     fn set_len(&mut self, _: usize) -> IoResult<()> {
//         Err(IoError::PermissionDenied)
//     }

//     // TODO: fn open(&self, mode: ...) -> ...

//     fn read(&self, offset: usize, buf: &mut [u8]) -> IoResult<usize> {
//         // FIXME: should be computed at `open`

//         // let pfa = &*hyperion_mem::pmm::PFA;

//         // let mut contents = String::new();
//         // writeln!(&mut contents, "MemTotal:{}", pfa.usable_mem() / 0x1000).unwrap();
//         // writeln!(&mut contents, "MemFree:{}", pfa.free_mem() / 0x1000).unwrap();

//         // <[u8]>::read(contents.as_bytes(), offset, buf)
//     }

//     fn write(&mut self, _: usize, _: &[u8]) -> IoResult<usize> {
//         Err(IoError::PermissionDenied)
//     }
// }

//

pub struct ProcFs;

impl<Mut: RawMutex> DirectoryDevice<Mut> for ProcFs {
    fn driver(&self) -> &'static str {
        "procfs"
    }

    fn get_node(&mut self, name: &str) -> IoResult<Node<Mut>> {
        match name {
            "meminfo" => {
                let pfa = &*hyperion_mem::pmm::PFA;

                let mut meminfo = File::new(&[]);
                let mut writer = (&mut meminfo as &mut dyn FileDevice).as_fmt(0);

                writeln!(writer, "MemTotal:{} kb", pfa.usable_mem() / 0x400);
                writeln!(writer, "MemFree: {} kb", pfa.free_mem() / 0x400);

                // create a snapshot of the system memory info to fix some data races
                Ok(Node::File(Arc::new(lock_api::Mutex::new(meminfo))))
            }
            _ => Err(IoError::NotFound),
        }
    }

    fn create_node(&mut self, name: &str, node: Node<Mut>) -> IoResult<()> {
        Err(IoError::PermissionDenied)
    }

    fn nodes(&mut self) -> IoResult<Arc<[Arc<str>]>> {
        Ok(["meminfo".into()].into())
    }
}
