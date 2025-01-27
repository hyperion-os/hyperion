use alloc::{
    boxed::Box,
    collections::{btree_map::Entry, BTreeMap},
    sync::Arc,
    vec,
    vec::Vec,
};
use core::{any::Any, mem, ops::Range};

use hyperion_arch::vmm::PageMap;
use hyperion_mem::{
    pmm::{PageFrame, PFA},
    vmm::{MapTarget, PageMapImpl},
};
use hyperion_scheduler::lock::Mutex;
use hyperion_syscall::err::{Error, Result};
use x86_64::{structures::paging::PageTableFlags, VirtAddr};

use crate::{
    device::{ArcOrRef, DirEntry, DirectoryDevice, FileDevice},
    tree::{DirRef, FileRef, Node, WeakDirRef},
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

    pub fn new_empty() -> FileRef {
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

pub struct Directory {
    pub name: Arc<str>,
    pub children: BTreeMap<Arc<str>, Node>,
    pub parent: Option<WeakDirRef>,

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

    fn set_len(&mut self, len: usize) -> Result<()> {
        self.len = len;
        Ok(())
    }

    fn map_phys(
        &mut self,
        vmm: &PageMap,
        v_addr: Range<VirtAddr>,
        flags: PageTableFlags,
    ) -> Result<usize> {
        if !v_addr.start.is_aligned(0x1000u64) {
            // FIXME: use the real abi error
            return Err(Error::PERMISSION_DENIED);
        }

        let mut pos = v_addr.start;

        // grow the file
        if (v_addr.end - v_addr.start) as usize > self.len {
            self.set_len((v_addr.end - v_addr.start) as usize)?;
        }

        // TODO: use lazy mapping instead
        // lazy allocate more pages, since the file size is larger
        // than there are physical pages currently allocated
        let allocated = self.pages.iter().map(|p| p.byte_len()).sum::<usize>();
        let needed = (v_addr.end - v_addr.start) as usize;
        if needed > allocated {
            let missing_pages = needed.abs_diff(allocated).div_ceil(0x1000);

            for _ in 0..missing_pages {
                let mut page = PFA.alloc(1);
                page.as_bytes_mut().fill(0);
                self.pages.push(page);
            }
        }

        for pages in self.pages.iter() {
            if pos + pages.byte_len() >= v_addr.end {
                vmm.map(
                    pos..v_addr.end,
                    MapTarget::Borrowed(pages.physical_addr()),
                    flags,
                );
                pos = v_addr.end;
                break;
            }

            vmm.map(
                pos..pos + pages.byte_len(),
                MapTarget::Borrowed(pages.physical_addr()),
                flags,
            );

            pos += pages.byte_len();
        }

        Ok((pos - v_addr.start) as usize)
    }

    fn unmap_phys(&mut self) -> Result<()> {
        Ok(())
    }

    fn read(&self, offset: usize, mut buf: &mut [u8]) -> Result<usize> {
        // FIXME: this should really just temporarily map the pages
        // its simpler and its faster and its less buggy

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

    fn write(&mut self, offset: usize, mut buf: &[u8]) -> Result<usize> {
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

    fn read(&self, offset: usize, buf: &mut [u8]) -> Result<usize> {
        self.bytes.read(offset, buf)
    }
}

impl DirectoryDevice for Directory {
    fn driver(&self) -> &'static str {
        "vfs"
    }

    fn get_node(&mut self, name: &str) -> Result<Node> {
        if let Some(node) = self.children.get(name) {
            Ok(node.clone())
        } else {
            Err(Error::NOT_FOUND)
        }
    }

    fn create_node(&mut self, name: &str, node: Node) -> Result<()> {
        match self.children.entry(name.into()) {
            Entry::Vacant(entry) => {
                entry.insert(node);
                self.nodes_cache = None;
                Ok(())
            }
            Entry::Occupied(_) => Err(Error::ALREADY_EXISTS),
        }
    }

    fn nodes(&mut self) -> Result<Box<dyn ExactSizeIterator<Item = DirEntry<'_>> + '_>> {
        Ok(Box::new(self.children.iter().map(|(name, node)| {
            DirEntry {
                name: ArcOrRef::Ref(name),
                node: node.clone(),
            }
        })))
    }
}

impl Directory {
    pub fn new(name: impl Into<Arc<str>>) -> Self {
        Self {
            name: name.into(),
            children: BTreeMap::new(),
            parent: None,

            nodes_cache: None,
        }
    }

    pub fn new_ref(name: impl Into<Arc<str>>) -> DirRef {
        Arc::new(Mutex::new(Self::new(name))) as _
    }
}
