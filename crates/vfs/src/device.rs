use alloc::sync::Arc;
use core::sync::atomic::{AtomicBool, Ordering};

use crossbeam::atomic::AtomicCell;
use hyperion_log::{debug, warn};

use crate::{
    error::{IoError, IoResult},
    tree::Node,
};

//

pub fn set_io_device_loader(f: fn()) {
    IO_DEVICES.store(f);
}

pub(crate) fn init() {
    static IO_DEVICES_ONCE: AtomicBool = AtomicBool::new(true);
    if IO_DEVICES_ONCE.swap(false, Ordering::Release) {
        debug!("Initializing VFS");
        IO_DEVICES.load()();
    }
}

//

pub trait FileDevice: Send + Sync {
    fn len(&self) -> usize;

    fn is_empty(&self) -> bool {
        self.len() == 0
    }

    fn read(&self, offset: usize, buf: &mut [u8]) -> IoResult<usize>;

    fn read_exact(&self, mut offset: usize, mut buf: &mut [u8]) -> IoResult<()> {
        while !buf.is_empty() {
            match self.read(offset, buf) {
                Ok(0) => break,
                Ok(n) => {
                    offset += n;
                    let tmp = buf;
                    buf = &mut tmp[n..];
                }
                Err(IoError::Interrupted) => {}
                Err(err) => return Err(err),
            }
        }

        if !buf.is_empty() {
            Err(IoError::UnexpectedEOF)
        } else {
            Ok(())
        }
    }

    fn write(&mut self, offset: usize, buf: &[u8]) -> IoResult<usize>;

    fn write_exact(&mut self, mut offset: usize, mut buf: &[u8]) -> IoResult<()> {
        while !buf.is_empty() {
            match self.write(offset, buf) {
                Ok(0) => return Err(IoError::WriteZero),
                Ok(n) => {
                    offset += n;
                    buf = &buf[n..];
                }
                Err(IoError::Interrupted) => {}
                Err(err) => return Err(err),
            }
        }
        Ok(())
    }
}

pub trait DirectoryDevice: Send + Sync {
    fn get_node(&mut self, name: &str) -> IoResult<Node>;

    fn create_node(&mut self, name: &str, node: Node) -> IoResult<()>;

    fn nodes(&mut self) -> IoResult<Arc<[Arc<str>]>>;
}

//

impl FileDevice for &'_ [u8] {
    fn len(&self) -> usize {
        <[u8]>::len(self)
    }

    fn read(&self, offset: usize, buf: &mut [u8]) -> IoResult<usize> {
        let len = self
            .len()
            .checked_sub(offset)
            .ok_or(IoError::UnexpectedEOF)?
            .min(buf.len());

        buf[..len].copy_from_slice(&self[offset..offset + len]);

        Ok(len)
    }

    fn write(&mut self, _: usize, _: &[u8]) -> IoResult<usize> {
        Err(IoError::PermissionDenied)
    }
}

impl FileDevice for &'_ mut [u8] {
    fn len(&self) -> usize {
        <[u8]>::len(self)
    }

    fn read(&self, offset: usize, buf: &mut [u8]) -> IoResult<usize> {
        let immut: &[u8] = &**self;
        immut.read(offset, buf)
    }

    fn write(&mut self, offset: usize, buf: &[u8]) -> IoResult<usize> {
        let len = self
            .len()
            .checked_sub(offset)
            .ok_or(IoError::UnexpectedEOF)?
            .min(buf.len());

        self[offset..offset + len].copy_from_slice(&buf[..len]);

        Ok(len)
    }
}

//

static IO_DEVICES: AtomicCell<fn()> =
    AtomicCell::new(|| warn!("Device provicer wasn't given to VFS"));
