use core::{
    any::Any,
    sync::atomic::{AtomicBool, Ordering},
};

use hyperion_syscall::err::{Error, Result};
use hyperion_vfs::{
    device::{DirectoryDevice, FileDevice},
    ramdisk::Directory,
    tree::Node,
};

//

pub fn sysctl_base() -> Node {
    let mut sys = Directory::new("sys");
    sys.create_node("kernel", sysctl_kernel()).unwrap();
    Node::new_dir(sys)
}

fn sysctl_kernel() -> Node {
    let mut group = Directory::new("kernel");
    group
        .create_node(
            "sched_rr_enabled",
            Node::new_file(SysCtlToggle {
                val: &hyperion_scheduler::ROUND_ROBIN,
            }),
        )
        .unwrap();
    Node::new_dir(group)
}

//

pub struct SysCtlToggle {
    val: &'static AtomicBool,
}

impl FileDevice for SysCtlToggle {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn len(&self) -> usize {
        1
    }

    fn read(&self, offset: usize, buf: &mut [u8]) -> Result<usize> {
        let val_bytes = if self.val.load(Ordering::SeqCst) {
            b"1"
        } else {
            b"0"
        };

        <[u8]>::read(val_bytes, offset, buf)
    }

    fn write(&mut self, offset: usize, buf: &[u8]) -> Result<usize> {
        if offset != 0 || buf.is_empty() {
            return Ok(0);
        }

        let mut val_bytes = *b"?";
        let n = <[u8]>::write(&mut val_bytes, offset, buf)?;

        let new_val = match &val_bytes {
            b"1" => true,
            b"0" => false,
            _ => return Err(Error::FILESYSTEM_ERROR),
        };

        self.val.store(new_val, Ordering::SeqCst);

        Ok(n)
    }
}

// //

// pub struct SysCtlGroup {

// }

// //

// impl DirectoryDevice for SysCtlGroup {
//     fn get_node(&mut self, name: &str) -> IoResult<Node> {
//         match name {
//             "kernel" => Ok(Node::File())
//             _ => Err(IoError::NotFound),
//         }
//     }

//     fn create_node(&mut self, name: &str, node: Node) -> IoResult<()> {
//         todo!()
//     }

//     fn nodes(&mut self) -> IoResult<Box<dyn ExactSizeIterator<Item = DirEntry<'_, Mut>> + '_>> {
//         todo!()
//     }
// }
