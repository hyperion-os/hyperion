use alloc::{
    boxed::Box,
    collections::{btree_map::Entry, BTreeMap},
    sync::Arc,
    vec,
    vec::Vec,
};
use core::{any::Any, mem};

use hyperion_mem::pmm::{PageFrame, PFA};
use lock_api::Mutex;

use crate::{
    device::{ArcOrRef, DirEntry, DirectoryDevice, FileDevice},
    error::{IoError, IoResult},
    tree::{DirRef, FileRef, Node, WeakDirRef},
    AnyMutex,
};

//

pub struct File {
    // bytes: Vec<u8>,
    pages: Vec<PageFrame>,
    len: usize,
}

impl File {
    pub fn new(bytes: &[u8]) -> Self {
        if bytes.is_empty() {
            Self {
                pages: vec![],
                len: 0,
            }
        } else {
            let pages = bytes.len().div_ceil(0x1000);
            let mut pages = PFA.alloc(pages);
            pages.as_bytes_mut()[..bytes.len()].copy_from_slice(bytes);

            Self {
                pages: vec![pages],
                len: bytes.len(),
            }
        }
    }

    pub fn new_empty<Mut: AnyMutex>() -> FileRef<Mut> {
        Arc::new(Mutex::new(Self {
            pages: Vec::new(),
            len: 0,
        })) as _
    }
}

impl Drop for File {
    fn drop(&mut self) {
        for page in mem::take(&mut self.pages) {
            page.free();
        }
    }
}

//

pub struct StaticRoFile {
    bytes: &'static [u8],
}

impl StaticRoFile {
    pub const fn new(bytes: &'static [u8]) -> Self {
        Self { bytes }
    }
}

pub struct Directory<Mut: AnyMutex> {
    pub name: Arc<str>,
    pub children: BTreeMap<Arc<str>, Node<Mut>>,
    pub parent: Option<WeakDirRef<Mut>>,

    nodes_cache: Option<Arc<[Arc<str>]>>,
}

//

impl FileDevice for File {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn len(&self) -> usize {
        self.len
    }

    fn set_len(&mut self, len: usize) -> IoResult<()> {
        self.len = len;
        Ok(())
    }

    fn map_phys(&mut self, min_bytes: usize) -> IoResult<Box<[PageFrame]>> {
        let mut pages_left = min_bytes.div_ceil(0x1000);
        let pages = self
            .pages
            .iter()
            .filter_map(move |page| {
                if pages_left == 0 {
                    return None;
                }

                let n = pages_left.min(page.len());
                pages_left = pages_left.saturating_sub(page.len());

                Some(unsafe { PageFrame::new(page.physical_addr(), n) })
            })
            .collect::<Box<[PageFrame]>>();

        Ok(pages)
    }

    fn unmap_phys(&mut self) -> IoResult<()> {
        Ok(())
    }

    fn read(&self, offset: usize, mut buf: &mut [u8]) -> IoResult<usize> {
        if let Some(buf_limit) = self.len.checked_sub(offset) {
            let buf_limit = buf_limit.min(buf.len());
            buf = &mut buf[..buf_limit];
        } else {
            return Ok(0);
        }
        let initial_len = buf.len();

        let mut current_page_start = 0usize;
        let mut pages = self.pages.iter();
        while !buf.is_empty() {
            // let limit = self.len - current_page_start;

            let Some(at) = pages.next() else {
                return Ok(initial_len - buf.len());
            };

            if let Some(read_from) = offset
                .checked_sub(current_page_start)
                .and_then(|read_from| at.as_bytes().get(read_from..))
            {
                let read_limit = read_from.len().min(buf.len());
                buf[..read_limit].copy_from_slice(&read_from[..read_limit]);
                buf = buf.split_at_mut(read_limit).1;
            }

            current_page_start += at.byte_len();
        }

        Ok(initial_len)
    }

    fn write(&mut self, offset: usize, mut buf: &[u8]) -> IoResult<usize> {
        self.len = self.len.max(offset + buf.len());

        let initial_len = buf.len();

        let mut current_page_start = 0usize;
        let mut pages = self.pages.iter_mut();
        while !buf.is_empty() {
            let Some(at) = pages.next() else {
                while !buf.is_empty() {
                    let mut page = PFA.alloc(1);
                    page.as_bytes_mut().fill(0);
                    let write_to = page.as_bytes_mut();

                    let write_limit = write_to.len().min(buf.len());
                    write_to[..write_limit].copy_from_slice(&buf[..write_limit]);
                    buf = buf.split_at(write_limit).1;

                    self.pages.push(page);
                }
                break;
            };

            if let Some(write_to) = offset
                .checked_sub(current_page_start)
                .and_then(|write_to| at.as_bytes_mut().get_mut(write_to..))
            {
                let write_limit = write_to.len().min(buf.len());
                write_to[..write_limit].copy_from_slice(&buf[..write_limit]);
                buf = buf.split_at(write_limit).1;
            }

            current_page_start += at.byte_len();
        }

        Ok(initial_len)
    }
}

impl FileDevice for StaticRoFile {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn len(&self) -> usize {
        self.bytes.len()
    }

    fn set_len(&mut self, _: usize) -> IoResult<()> {
        Err(IoError::PermissionDenied)
    }

    fn read(&self, offset: usize, buf: &mut [u8]) -> IoResult<usize> {
        self.bytes.read(offset, buf)
    }

    fn write(&mut self, _: usize, _: &[u8]) -> IoResult<usize> {
        Err(IoError::PermissionDenied)
    }
}

impl<Mut: AnyMutex> DirectoryDevice<Mut> for Directory<Mut> {
    fn driver(&self) -> &'static str {
        "vfs"
    }

    fn get_node(&mut self, name: &str) -> IoResult<Node<Mut>> {
        if let Some(node) = self.children.get(name) {
            Ok(node.clone())
        } else {
            Err(IoError::NotFound)
        }
    }

    fn create_node(&mut self, name: &str, node: Node<Mut>) -> IoResult<()> {
        match self.children.entry(name.into()) {
            Entry::Vacant(entry) => {
                entry.insert(node);
                self.nodes_cache = None;
                Ok(())
            }
            Entry::Occupied(_) => Err(IoError::AlreadyExists),
        }
    }

    fn nodes(&mut self) -> IoResult<Box<dyn ExactSizeIterator<Item = DirEntry<'_, Mut>> + '_>> {
        Ok(Box::new(self.children.iter().map(|(name, node)| {
            DirEntry {
                name: ArcOrRef::Ref(name),
                node: node.clone(),
            }
        })))
    }
}

impl<Mut: AnyMutex> Directory<Mut> {
    pub fn new(name: impl Into<Arc<str>>) -> Self {
        Self {
            name: name.into(),
            children: BTreeMap::new(),
            parent: None,

            nodes_cache: None,
        }
    }

    pub fn new_ref(name: impl Into<Arc<str>>) -> DirRef<Mut> {
        Arc::new(Mutex::new(Self::new(name))) as _
    }
}
