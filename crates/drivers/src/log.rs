use alloc::boxed::Box;
use core::{any::Any, mem::MaybeUninit, str::from_utf8};

use async_trait::async_trait;
use hyperion_arch::vmm::PageMap;
use hyperion_log::*;
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
                let bytes = unsafe { MaybeUninit::slice_assume_init_ref(slice) };
                let str = core::str::from_utf8(bytes).map_err(|_| Error::INVALID_UTF8)?;
                hyperion_log::print!("{str}");
                Ok(())
            })?;
        }

        Ok(buf.len())
    }

    // fn as_any(&self) -> &dyn Any {
    //     self
    // }

    // fn len(&self) -> usize {
    //     0
    // }

    // fn set_len(&mut self, _: usize) -> IoResult<()> {
    //     Err(IoError::PermissionDenied)
    // }

    // fn read(&self, _: usize, _: &mut [u8]) -> IoResult<usize> {
    //     Ok(0)
    // }

    // fn write(&mut self, _: usize, buf: &[u8]) -> IoResult<usize> {
    //     if let Ok(str) = from_utf8(buf) {
    //         print!("{str}");
    //     }

    //     Ok(buf.len())
    // }
}
