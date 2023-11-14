#![no_std]

use hyperion_scheduler::lock::Futex;
use hyperion_vfs::tree::Node;
use spin::Lazy;

//

pub static VFS_ROOT: Lazy<Node<Futex>> = Lazy::new(Node::new_root);
