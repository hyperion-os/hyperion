use core::sync::atomic::AtomicBool;

use hyperion_vfs::{
    device::DirectoryDevice,
    error::IoResult,
    tree::{IntoNode, Node},
};

//

pub fn init(root: impl IntoNode) {
    root.into_node().mount("sys", SysFs::new());
}

//

struct SysFs {}

impl SysFs {
    fn new() -> Self {
        Self {}
    }
}

impl DirectoryDevice for SysFs {
    fn get_node(&mut self, name: &str) -> IoResult<Node<Mut>> {}

    fn create_node(&mut self, name: &str, node: Node<Mut>) -> IoResult<()> {
        todo!()
    }

    fn nodes(&mut self) -> IoResult<Box<dyn ExactSizeIterator<Item = DirEntry<'_, Mut>> + '_>> {
        todo!()
    }
}

//

struct Toggle {
    v: &'static AtomicBool,
}
