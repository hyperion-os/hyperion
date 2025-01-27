#![no_std]

//

extern crate alloc;

pub use hyperion_syscall::err::{Error, Result};
use lock_api::RawMutex;

use crate::device::FileDevice;

//

// pub struct TestNode<Mut: VfsMutex<Self>> {
//     branches: Vec<Mut>,
// }

pub trait AnyMutex: RawMutex + Send + Sync + 'static {}

impl<T> AnyMutex for T where T: RawMutex + Send + Sync + 'static {}

//

pub mod device;
pub mod path;
pub mod ramdisk;
pub mod tree;

//

// pub fn get_root() -> Node {
//     pub static ROOT: Lazy<Root> = Lazy::new(|| Directory::new_ref(""));
//     let root = ROOT.clone();

//     device::init();

//     Node::Directory(root)
// }

// pub fn get_node(path: impl AsRef<Path>, make_dirs: bool) -> IoResult<Node> {
//     get_node_with(get_root(), path, make_dirs)
// }

// pub fn get_dir(path: impl AsRef<Path>, make_dirs: bool) -> IoResult<DirRef> {
//     get_dir_with(get_root(), path, make_dirs)
// }

// // TODO: create
// pub fn get_file(path: impl AsRef<Path>, make_dirs: bool, create: bool) -> IoResult<FileRef> {
//     get_file_with(get_root(), path, make_dirs, create)
// }

// pub fn create_device(path: impl AsRef<Path>, make_dirs: bool, dev: FileRef) -> IoResult<()> {
//     create_device_with(get_root(), path, make_dirs, dev)
// }

// pub fn install_dev(path: impl AsRef<Path>, dev: impl FileDevice + 'static) {
//     install_dev_with(get_root(), path, dev)
// }

// pub use get_dir as read_dir;
// pub use get_file as open;

//
