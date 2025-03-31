use alloc::boxed::Box;

use async_trait::async_trait;
use hyperion_arch::vmm::PageMap;
use hyperion_mem::buf::{Buffer, BufferMut};
use hyperion_scheduler::proc::Process;
use hyperion_syscall::err::{Error, Result};
use hyperion_vfs::node::{FileDriver, Ref};

//

pub static DEV_LOG: Ref<dyn FileDriver> = Ref::new_static(&KernelLogs);

//

/// `/dev/log` which prints to the kernel serial logs
pub struct KernelLogs;

#[async_trait]
impl FileDriver for KernelLogs {
    async fn read(
        &self,
        _: Option<&Process>,
        _: usize,
        _: BufferMut<'_, u8, PageMap>,
    ) -> Result<usize> {
        Err(Error::PERMISSION_DENIED)
    }

    async fn write(
        &self,
        _: Option<&Process>,
        _: usize,
        buf: Buffer<'_, u8, PageMap>,
    ) -> Result<usize> {
        unsafe {
            buf.with_slice(|slice| {
                let bytes = slice.assume_init_ref();
                let str = core::str::from_utf8(bytes).map_err(|_| Error::INVALID_UTF8)?;
                hyperion_log::print!("{str}");
                Ok(())
            })?;
        }

        Ok(buf.len())
    }
}
