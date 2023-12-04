use alloc::{
    collections::{btree_map::Entry, BTreeMap},
    sync::Arc,
    vec::Vec,
};

use lock_api::Mutex;

use crate::{
    device::{DirectoryDevice, FileDevice},
    error::{IoError, IoResult},
    tree::{DirRef, FileRef, Node, WeakDirRef},
    AnyMutex,
};

//

pub struct File {
    bytes: Vec<u8>,
}

impl File {
    pub fn new(bytes: Vec<u8>) -> Self {
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
    fn as_any(&self) -> &dyn core::any::Any {
        self
    }

    fn len(&self) -> usize {
        self.bytes.len()
    }

    fn read(&self, offset: usize, buf: &mut [u8]) -> IoResult<usize> {
        FileDevice::read(&self.bytes[..], offset, buf)
    }

    fn write(&mut self, offset: usize, buf: &[u8]) -> IoResult<usize> {
        self.bytes
            .resize(self.bytes.len().max(buf.len() + offset), b'?');
        FileDevice::write(&mut self.bytes[..], offset, buf)
    }
}

impl File {
    pub fn new_empty<Mut: AnyMutex>() -> FileRef<Mut> {
        Arc::new(Mutex::new(Self { bytes: Vec::new() })) as _
    }
}

impl<Mut: AnyMutex> DirectoryDevice<Mut> for Directory<Mut> {
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

    fn nodes(&mut self) -> IoResult<Arc<[Arc<str>]>> {
        Ok(self
            .nodes_cache
            .get_or_insert_with(|| self.children.keys().cloned().collect())
            .clone())
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
