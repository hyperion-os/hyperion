use alloc::{boxed::Box, sync::Arc};
use core::{
    any::Any,
    fmt,
    ops::{Deref, Range},
};

use async_trait::async_trait;
use hyperion_arch::vmm::PageMap;
use lock_api::RawMutex;
use x86_64::{structures::paging::PageTableFlags, VirtAddr};

use crate::{
    error::{IoError, IoResult},
    tree::Node,
};

//

#[derive(Debug, Clone, Copy)]
pub struct PhysicalAddress(pub usize);

//

#[async_trait]
pub trait FileDevice: Send + Sync {
    fn driver(&self) -> &'static str {
        "unknown"
    }

    /// is the file size 0 bytes?
    async fn is_empty(&self) -> bool {
        self.len().await == 0
    }

    /// file size in bytes
    async fn len(&self) -> usize;
    // async fn len(&self) -> usize {
    //     Err(IoError::PermissionDenied)
    // }

    /// truncate or add zeros to set the length
    async fn set_len(&self, len: usize) -> IoResult<()> {
        _ = len;
        Err(IoError::PermissionDenied)
    }

    // /// Is the underlying file data already in a physical address?
    // /// If so, where?
    // async fn memory_mapped_to() -> Option<PhysicalAddress> {}

    /* /// If the file data isn't already memory mapped, map it here
    async fn memory_map_to(to: &mut [u8]) ->

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
    ) -> IoResult<usize> {
        _ = (vmm, v_addr, flags);
        Err(IoError::PermissionDenied)
    }

    /// see [`Self::map_phys`]
    async fn unmap_phys(&self) -> IoResult<()> {
        Err(IoError::PermissionDenied)
    } */

    async fn read(&self, offset: usize, buf: &mut [u8]) -> IoResult<usize> {
        _ = (offset, buf);
        Err(IoError::PermissionDenied)
    }

    async fn write(&self, offset: usize, buf: &[u8]) -> IoResult<usize> {
        _ = (offset, buf);
        Err(IoError::PermissionDenied)
    }

    async fn read_exact(&self, mut offset: usize, mut buf: &mut [u8]) -> IoResult<()> {
        while !buf.is_empty() {
            match self.read(offset, buf).await {
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

    async fn write_exact(&self, mut offset: usize, mut buf: &[u8]) -> IoResult<()> {
        while !buf.is_empty() {
            match self.write(offset, buf).await {
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

async fn default_read_exact() {}

#[async_trait]
pub trait DirectoryDevice: Send + Sync {
    fn driver(&self) -> &'static str {
        "unknown"
    }

    /// get an entry
    async fn get(&self, name: &str) -> IoResult<Node> {
        _ = name;
        Err(IoError::PermissionDenied)
    }

    /// get or insert an entry
    async fn get_or_insert(&self, name: &str) -> IoResult<Node> {
        _ = name;
        Err(IoError::PermissionDenied)
    }

    /// insert an entry
    async fn insert(&self, name: &str, node: Node) -> IoResult<()> {
        _ = (name, node);
        Err(IoError::PermissionDenied)
    }

    /// remove an entry
    async fn remove(&self, name: &str) -> IoResult<Node> {
        _ = name;
        Err(IoError::PermissionDenied)
    }

    async fn entries(&self, callback: &mut dyn FnMut(DirEntry) -> bool) -> IoResult<()> {
        _ = callback;
        Err(IoError::PermissionDenied)
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

#[async_trait]
impl FileDevice for [u8] {
    async fn len(&self) -> usize {
        <[u8]>::len(self)
    }

    async fn read(&self, offset: usize, buf: &mut [u8]) -> IoResult<usize> {
        let len = self
            .len()
            .checked_sub(offset)
            .ok_or(IoError::UnexpectedEOF)?
            .min(buf.len());

        buf[..len].copy_from_slice(&self[offset..][..len]);

        Ok(len)
    }

    async fn write(&mut self, offset: usize, buf: &[u8]) -> IoResult<usize> {
        let len = self
            .len()
            .checked_sub(offset)
            .ok_or(IoError::UnexpectedEOF)?
            .min(buf.len());

        self[offset..][..len].copy_from_slice(&buf[..len]);

        Ok(len)
    }
}

#[async_trait]
impl<T: FileDevice> FileDevice for &'static T {
    async fn len(&self) -> usize {
        (**self).len().await
    }

    async fn read(&self, offset: usize, buf: &mut [u8]) -> IoResult<usize> {
        (**self).read(offset, buf).await
    }
}

//

// pub struct FmtWriteFile<'a>(&'a mut dyn FileDevice, usize);

// impl dyn FileDevice {
//     pub fn as_fmt(&mut self, at: usize) -> FmtWriteFile {
//         FmtWriteFile(self, at)
//     }
// }

// impl fmt::Write for FmtWriteFile<'_> {
//     fn write_str(&mut self, s: &str) -> fmt::Result {
//         if let Ok(n) = self.0.write(self.1, s.as_bytes()) {
//             self.1 += n;
//             Ok(())
//         } else {
//             Err(fmt::Error)
//         }
//     }
// }
