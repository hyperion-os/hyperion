use alloc::boxed::Box;
use core::marker::PhantomData;

use hyperion_vfs::{
    device::{DirEntry, DirectoryDevice},
    error::{IoError, IoResult},
    tree::{IntoNode, Node},
    AnyMutex,
};

//

pub fn init(root: impl IntoNode) {
    root.into_node().mount("init", InitFs::new());
}

//

struct InitFs<Mut> {
    _p: PhantomData<Mut>,
}

impl<Mut: AnyMutex> InitFs<Mut> {
    fn new() -> Self {
        Self { _p: PhantomData }
    }
}

impl<Mut: AnyMutex> DirectoryDevice<Mut> for InitFs<Mut> {
    fn driver(&self) -> &'static str {
        "initfs"
    }

    fn get_node(&mut self, name: &str) -> IoResult<Node<Mut>> {
        _ = name;
        Err(IoError::PermissionDenied)
    }

    fn create_node(&mut self, name: &str, node: Node<Mut>) -> IoResult<()> {
        _ = (name, node);
        Err(IoError::PermissionDenied)
    }

    fn nodes(&mut self) -> IoResult<Box<dyn ExactSizeIterator<Item = DirEntry<'_, Mut>> + '_>> {
        Err(IoError::PermissionDenied)
    }
}
