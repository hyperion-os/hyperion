use core::{any::Any, str::from_utf8};

use hyperion_log::println;
use hyperion_vfs::{
    device::FileDevice,
    error::{IoError, IoResult},
};

//

/// `/dev/log` which prints to the kernel serial logs
pub struct KernelLogs;

impl FileDevice for KernelLogs {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn len(&self) -> usize {
        0
    }

    fn set_len(&mut self, _: usize) -> IoResult<()> {
        Err(IoError::PermissionDenied)
    }

    fn read(&self, _: usize, _: &mut [u8]) -> IoResult<usize> {
        Ok(0)
    }

    fn write(&mut self, _: usize, buf: &[u8]) -> IoResult<usize> {
        if let Ok(str) = from_utf8(buf) {
            println!("{str}");
        }

        Ok(buf.len())
    }
}
