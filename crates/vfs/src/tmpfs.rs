use alloc::{boxed::Box, collections::btree_map::BTreeMap, sync::Arc};

use async_trait::async_trait;
use hyperion_futures::lock::Mutex;
use hyperion_scheduler::proc::Process;
use hyperion_syscall::err::{Error, Result};

use crate::node::{CacheAllowed, DirDriver, DirNode, FileDriver, FileNode, Node, Ref};

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
        let new_node = Node::Dir(Ref::from_arc(Arc::new(DirNode {
            nodes: Mutex::new(BTreeMap::new()),
            driver: Mutex::new(Ref::from_arc(Arc::new(Self::new()) as _)),
        })));

        let mut nodes = self.nodes.lock().await;
        nodes.insert(name.into(), new_node.clone());
        Ok((new_node, true))
    }

    /// create a new file in this directory and return a cache Node
    async fn create_file(&self, _: Option<&Process>, name: &str) -> Result<(Node, CacheAllowed)> {
        let new_node = Node::File(Ref::from_arc(Arc::new(FileNode {
            driver: Mutex::new(Ref::from_arc(Arc::new(TmpFsFile {}) as _)),
        })));

        let mut nodes = self.nodes.lock().await;
        nodes.insert(name.into(), new_node.clone());
        Ok((new_node, true))
    }
}

//

pub struct TmpFsFile {}

impl FileDriver for TmpFsFile {}
