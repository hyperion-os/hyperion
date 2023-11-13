use alloc::{
    collections::{btree_map::Entry, BTreeMap},
    sync::Arc,
};

use spin::Mutex;

use crate::{
    device::{DirectoryDevice, FileDevice},
    error::{IoError, IoResult},
    tree::{DirRef, FileRef, Node, Root, WeakDirRef},
};

//

pub struct File {}

pub struct Directory {
    pub name: Arc<str>,
    pub children: BTreeMap<Arc<str>, Node>,
    pub parent: Option<WeakDirRef>,

    nodes_cache: Option<Arc<[Arc<str>]>>,
}

//

impl FileDevice for File {
    fn len(&self) -> usize {
        0
    }

    fn read(&self, offset: usize, buf: &mut [u8]) -> IoResult<usize> {
        FileDevice::read(&[][..], offset, buf)
    }

    fn write(&mut self, offset: usize, buf: &[u8]) -> IoResult<usize> {
        FileDevice::write(&mut [][..], offset, buf)
    }
}

impl File {
    pub fn new_empty() -> FileRef {
        Arc::new(Mutex::new(Self {})) as _
    }
}

impl DirectoryDevice for Directory {
    fn get_node(&mut self, name: &str) -> IoResult<Node> {
        if let Some(node) = self.children.get(name) {
            Ok(node.clone())
        } else {
            Err(IoError::NotFound)
        }
    }

    fn create_node(&mut self, name: &str, node: Node) -> IoResult<()> {
        match self.children.entry(name.into()) {
            Entry::Vacant(entry) => {
                entry.insert(node);
                self.nodes_cache = None;
                Ok(())
            }
            Entry::Occupied(_) => Err(IoError::AlreadyExists),
        }
    }

    fn nodes(&mut self) -> IoResult<Arc<[Arc<str>]>> {
        Ok(self
            .nodes_cache
            .get_or_insert_with(|| self.children.keys().cloned().collect())
            .clone())
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

    pub fn root() -> Root {
        Self::new_ref("")
    }
}
