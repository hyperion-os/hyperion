use alloc::string::String;
use core::{any::Any, fmt::Write};

use hyperion_vfs::{
    device::FileDevice,
    error::{IoError, IoResult},
    tree::IntoNode,
};

//

pub fn init(root: impl IntoNode) {
    let root = root.into_node().find("/proc", true).unwrap();

    root.install_dev("meminfo", MemInfo);
}

//

pub struct MemInfo;

impl FileDevice for MemInfo {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn len(&self) -> usize {
        1
    }

    fn set_len(&mut self, _: usize) -> IoResult<()> {
        Err(IoError::PermissionDenied)
    }

    // TODO: fn open(&self, mode: ...) -> ...

    fn read(&self, offset: usize, buf: &mut [u8]) -> IoResult<usize> {
        // FIXME: should be computed at `open`

        let pfa = &*hyperion_mem::pmm::PFA;

        let mut contents = String::new();
        writeln!(&mut contents, "MemTotal:{}", pfa.usable_mem() / 0x1000).unwrap();
        writeln!(&mut contents, "MemFree:{}", pfa.free_mem() / 0x1000).unwrap();

        <[u8]>::read(contents.as_bytes(), offset, buf)
    }

    fn write(&mut self, _: usize, _: &[u8]) -> IoResult<usize> {
        Err(IoError::PermissionDenied)
    }
}

//

// TODO: pub struct ProcFs {}

// impl DirectoryDevice for ProcFs {}
