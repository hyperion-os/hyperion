use alloc::{boxed::Box, sync::Arc};
use core::{
    any::Any,
    fmt,
    ops::{Deref, Range},
};

use hyperion_arch::vmm::PageMap;
use hyperion_syscall::err::{Error, Result};
use x86_64::{structures::paging::PageTableFlags, VirtAddr};

use crate::tree::Node;

//

pub trait FileDevice: Send + Sync {
    fn driver(&self) -> &'static str {
        "unknown"
    }

    fn as_any(&self) -> &dyn Any;

    fn len(&self) -> usize;

    /// truncate or add zeros to set the length
    fn set_len(&mut self, len: usize) -> Result<()> {
        _ = len;
        Err(Error::PERMISSION_DENIED)
    }

    fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// allocate physical pages + map the file to it OR get the device physical address
    ///
    /// allocated pages are managed by this FileDevice, each [`Self::map_phys`] is paired with
    /// an [`Self::unmap_phys`] and only the last [`Self::unmap_phys`] deallocate the pages
    ///
    /// v_addr is for the map placement and its maximum size
    ///
    /// flags are the flags for each page
    ///
    /// returns the number of bytes actually mapped
    fn map_phys(
        &mut self,
        vmm: &PageMap,
        v_addr: Range<VirtAddr>,
        flags: PageTableFlags,
    ) -> Result<usize> {
        _ = (vmm, v_addr, flags);
        Err(Error::PERMISSION_DENIED)
    }

    /// see [`Self::map_phys`]
    fn unmap_phys(&mut self) -> Result<()> {
        Err(Error::PERMISSION_DENIED)
    }

    fn read(&self, offset: usize, buf: &mut [u8]) -> Result<usize> {
        _ = (offset, buf);
        Err(Error::PERMISSION_DENIED)
    }

    fn read_exact(&self, mut offset: usize, mut buf: &mut [u8]) -> Result<()> {
        while !buf.is_empty() {
            match self.read(offset, buf) {
                Ok(0) => break,
                Ok(n) => {
                    offset += n;
                    let tmp = buf;
                    buf = &mut tmp[n..];
                }
                Err(Error::INTERRUPTED) => {}
                Err(err) => return Err(err),
            }
        }

        if !buf.is_empty() {
            Err(Error::UNEXPECTED_EOF)
        } else {
            Ok(())
        }
    }

    fn write(&mut self, offset: usize, buf: &[u8]) -> Result<usize> {
        _ = (offset, buf);
        Err(Error::PERMISSION_DENIED)
    }

    fn write_exact(&mut self, mut offset: usize, mut buf: &[u8]) -> Result<()> {
        while !buf.is_empty() {
            match self.write(offset, buf) {
                Ok(0) => return Err(Error::WRITE_ZERO),
                Ok(n) => {
                    offset += n;
                    buf = &buf[n..];
                }
                Err(Error::INTERRUPTED) => {}
                Err(err) => return Err(err),
            }
        }
        Ok(())
    }
}

pub trait DirectoryDevice: Send + Sync {
    fn driver(&self) -> &'static str {
        "unknown"
    }

    fn get_node(&mut self, name: &str) -> Result<Node> {
        _ = name;
        Err(Error::PERMISSION_DENIED)
    }

    fn create_node(&mut self, name: &str, node: Node) -> Result<()> {
        _ = (name, node);
        Err(Error::PERMISSION_DENIED)
    }

    fn nodes(&mut self) -> Result<Box<dyn ExactSizeIterator<Item = DirEntry<'_>> + '_>> {
        Err(Error::PERMISSION_DENIED)
    }
}

//

pub struct DirEntry<'a> {
    pub name: ArcOrRef<'a, str>,
    pub node: Node,
}

pub enum ArcOrRef<'a, T: ?Sized> {
    Arc(Arc<T>),
    Ref(&'a T),
}

impl<'a, T: ?Sized> Deref for ArcOrRef<'a, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        match self {
            ArcOrRef::Arc(r) => r.as_ref(),
            ArcOrRef::Ref(r) => r,
        }
    }
}

//

impl FileDevice for [u8] {
    fn as_any(&self) -> &dyn Any {
        panic!()
    }

    fn len(&self) -> usize {
        <[u8]>::len(self)
    }

    fn set_len(&mut self, _: usize) -> Result<()> {
        Err(Error::PERMISSION_DENIED)
    }

    fn read(&self, offset: usize, buf: &mut [u8]) -> Result<usize> {
        let len = self
            .len()
            .checked_sub(offset)
            .ok_or(Error::UNEXPECTED_EOF)?
            .min(buf.len());

        buf[..len].copy_from_slice(&self[offset..][..len]);

        Ok(len)
    }

    fn write(&mut self, offset: usize, buf: &[u8]) -> Result<usize> {
        let len = self
            .len()
            .checked_sub(offset)
            .ok_or(Error::UNEXPECTED_EOF)?
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

    fn set_len(&mut self, _: usize) -> Result<()> {
        Err(Error::PERMISSION_DENIED)
    }

    fn read(&self, offset: usize, buf: &mut [u8]) -> Result<usize> {
        (**self).read(offset, buf)
    }

    fn write(&mut self, _: usize, _: &[u8]) -> Result<usize> {
        Err(Error::PERMISSION_DENIED)
    }
}

//

pub struct FmtWriteFile<'a>(&'a mut dyn FileDevice, usize);

impl dyn FileDevice {
    pub fn as_fmt(&mut self, at: usize) -> FmtWriteFile {
        FmtWriteFile(self, at)
    }
}

impl fmt::Write for FmtWriteFile<'_> {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        if let Ok(n) = self.0.write(self.1, s.as_bytes()) {
            self.1 += n;
            Ok(())
        } else {
            Err(fmt::Error)
        }
    }
}
