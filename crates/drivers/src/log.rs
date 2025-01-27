use core::{any::Any, str::from_utf8};

use hyperion_log::*;
use hyperion_vfs::{device::FileDevice, Result};

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

    fn write(&mut self, _: usize, buf: &[u8]) -> Result<usize> {
        if let Ok(str) = from_utf8(buf) {
            print!("{str}");
        }

        Ok(buf.len())
    }
}
