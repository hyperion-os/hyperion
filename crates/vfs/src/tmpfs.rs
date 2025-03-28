use alloc::{boxed::Box, collections::btree_map::BTreeMap, sync::Arc, vec::Vec};
use core::{mem::MaybeUninit, ptr, slice};

use async_trait::async_trait;
use hyperion_arch::vmm::PageMap;
use hyperion_futures::mutex::Mutex;
use hyperion_mem::{
    buf::{Buffer, BufferMut},
    pmm::PageFrame,
};
use hyperion_scheduler::proc::Process;
use hyperion_syscall::err::{Error, Result};

use crate::node::{
    CacheAllowed, DirDriver, DirDriverExt, DirNode, FileDriver, FileDriverExt, FileNode, Node, Ref,
};

//

pub struct TmpFs {
    nodes: Mutex<BTreeMap<Arc<str>, Node>>,
}

impl TmpFs {
    pub const fn new() -> Self {
        Self {
            nodes: Mutex::new(BTreeMap::new()),
        }
    }
}

impl Default for TmpFs {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl DirDriver for TmpFs {
    /// get a sub-directory or file in this directory as a cache Node
    async fn get(&self, _: Option<&Process>, name: &str) -> Result<(Node, CacheAllowed)> {
        let nodes = self.nodes.lock().await;
        nodes
            .get(name)
            .ok_or(Error::NOT_FOUND)
            .map(|node| (node.clone(), true))
    }

    /// create a new sub-directory in this directory and return a cache Node
    async fn create_dir(&self, _: Option<&Process>, name: &str) -> Result<(Node, CacheAllowed)> {
        let new_node = TmpFs::new().into_node();

        let mut nodes = self.nodes.lock().await;
        nodes.insert(name.into(), new_node.clone());
        Ok((new_node, true))
    }

    /// create a new file in this directory and return a cache Node
    async fn create_file(&self, _: Option<&Process>, name: &str) -> Result<(Node, CacheAllowed)> {
        let new_node = TmpFsFile::new().into_node();

        let mut nodes = self.nodes.lock().await;
        nodes.insert(name.into(), new_node.clone());
        Ok((new_node, true))
    }
}

//

pub struct TmpFsFile {
    pages: Mutex<Vec<PageFrame>>,
}

impl TmpFsFile {
    pub const fn new() -> Self {
        Self {
            pages: Mutex::new(Vec::new()),
        }
    }
}

impl Default for TmpFsFile {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl FileDriver for TmpFsFile {
    async fn read(
        &self,
        proc: Option<&Process>,
        offset: usize,
        mut buf: BufferMut<'_, u8, PageMap>,
    ) -> Result<usize> {
        let first = offset >> 12;
        let last = offset.saturating_add(buf.len()) >> 12;
        let count = last - first + 1;

        let mut pages = self.pages.lock().await;

        let mut pages = pages.iter().skip(last).take(count);

        let mut read = 0usize;

        // read the first (and maybe the last) (maybe partial) page
        if let Some(page) = pages.next() {
            let offset = offset & ((1 << 12) - 1);
            let from = page.virtual_addr().as_ptr::<MaybeUninit<u8>>();
            let from = unsafe { slice::from_raw_parts(from, 0x1000) };
            let from = &from[offset..];

            unsafe {
                buf.with_slice_mut(|s| {
                    let limit = s.len().min(from.len());
                    s[..limit].copy_from_slice(&from[..limit]);
                    read += limit;
                });
            }
        }

        // all other target pages have an aligned read

        // read the last (maybe partial) page
        if let Some(page) = pages.next_back() {
            let from = page.virtual_addr().as_ptr::<MaybeUninit<u8>>();
            let from = unsafe { slice::from_raw_parts(from, 0x1000) };

            unsafe {
                buf.with_slice_mut(|s| {
                    let limit = s.len().min(from.len());
                    s[read..][..limit].copy_from_slice(&from[..limit]);
                    read += limit;
                });
            }
        }

        // read the full pages in the middle
        for page in pages {
            let from = page.virtual_addr().as_ptr::<MaybeUninit<u8>>();
            let from = unsafe { slice::from_raw_parts(from, 0x1000) };

            unsafe {
                buf.with_slice_mut(|s| {
                    s[read..].copy_from_slice(from);
                    read += 0x1000;
                });
            }
        }

        Ok(read)
    }

    async fn write(
        &self,
        proc: Option<&Process>,
        offset: usize,
        buf: Buffer<'_, u8, PageMap>,
    ) -> Result<usize> {
        let first = offset >> 12;
        let last = offset.saturating_add(buf.len()) >> 12;
        let count = last - first + 1;

        let mut pages = self.pages.lock().await;
        pages.resize_with(last + 1, || hyperion_mem::pmm::PFA.alloc(1));

        let mut pages = pages.iter().skip(last).take(count);

        let mut written = 0usize;

        // write the first (and maybe the last) (maybe partial) page
        if let Some(page) = pages.next() {
            let offset = offset & ((1 << 12) - 1);
            let from = page.virtual_addr().as_mut_ptr::<MaybeUninit<u8>>();
            let from = unsafe { slice::from_raw_parts_mut(from, 0x1000) };
            let from = &mut from[offset..];

            unsafe {
                buf.with_slice(|s| {
                    let limit = s.len().min(from.len());
                    from[..limit].copy_from_slice(&s[..limit]);
                    written += limit;
                });
            }
        }

        // all other target pages have an aligned write

        // write the last (maybe partial) page
        if let Some(page) = pages.next_back() {
            let from = page.virtual_addr().as_mut_ptr::<MaybeUninit<u8>>();
            let from = unsafe { slice::from_raw_parts_mut(from, 0x1000) };

            unsafe {
                buf.with_slice(|s| {
                    let limit = s.len().min(from.len());
                    from[..limit].copy_from_slice(&s[written..][..limit]);
                    written += limit;
                });
            }
        }

        // write the full pages in the middle
        for page in pages {
            let from = page.virtual_addr().as_mut_ptr::<MaybeUninit<u8>>();
            let from = unsafe { slice::from_raw_parts_mut(from, 0x1000) };

            unsafe {
                buf.with_slice(|s| {
                    from.copy_from_slice(&s[written..]);
                    written += 0x1000;
                });
            }
        }

        Ok(written)
    }
}
