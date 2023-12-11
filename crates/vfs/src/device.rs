use alloc::sync::Arc;
use core::any::Any;

use lock_api::RawMutex;

use crate::{
    error::{IoError, IoResult},
    tree::Node,
};

//

pub trait FileDevice: Send + Sync {
    fn as_any(&self) -> &dyn Any;

    fn len(&self) -> usize;

    fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// allocate physical pages + map the file to it OR get the device physical address
    ///
    /// allocated pages are managed by this FileDevice, each [`Self::map_phys`] is paired with
    /// an [`Self::unmap_phys`] and only the last [`Self::unmap_phys`] deallocate the pages
    fn map_phys(&mut self, size_bytes: usize) -> IoResult<usize> {
        _ = size_bytes;
        Err(IoError::PermissionDenied)
    }

    /// see [`Self::map_phys`]
    fn unmap_phys(&mut self) -> IoResult<()> {
        Err(IoError::PermissionDenied)
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

pub trait DirectoryDevice<Mut: RawMutex>: Send + Sync {
    fn get_node(&mut self, name: &str) -> IoResult<Node<Mut>>;

    fn create_node(&mut self, name: &str, node: Node<Mut>) -> IoResult<()>;

    fn nodes(&mut self) -> IoResult<Arc<[Arc<str>]>>;
}

//

impl FileDevice for [u8] {
    fn as_any(&self) -> &dyn Any {
        panic!()
    }

    fn len(&self) -> usize {
        <[u8]>::len(self)
    }

    fn read(&self, offset: usize, buf: &mut [u8]) -> IoResult<usize> {
        let len = self
            .len()
            .checked_sub(offset)
            .ok_or(IoError::UnexpectedEOF)?
            .min(buf.len());

        buf[..len].copy_from_slice(&self[offset..][..len]);

        Ok(len)
    }

    fn write(&mut self, offset: usize, buf: &[u8]) -> IoResult<usize> {
        let len = self
            .len()
            .checked_sub(offset)
            .ok_or(IoError::UnexpectedEOF)?
            .min(buf.len());

        self[offset..][..len].copy_from_slice(&buf[..len]);

        Ok(len)
    }
}

impl<T: FileDevice> FileDevice for &'static T {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn len(&self) -> usize {
        (**self).len()
    }

    fn read(&self, offset: usize, buf: &mut [u8]) -> IoResult<usize> {
        (**self).read(offset, buf)
    }

    fn write(&mut self, _: usize, _: &[u8]) -> IoResult<usize> {
        Err(IoError::PermissionDenied)
    }
}
