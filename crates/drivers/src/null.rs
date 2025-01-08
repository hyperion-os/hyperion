use alloc::boxed::Box;
use core::{any::Any, mem::MaybeUninit};

use async_trait::async_trait;
use hyperion_arch::vmm::PageMap;
use hyperion_mem::buf::{Buffer, BufferMut};
use hyperion_scheduler::proc::Process;
use hyperion_syscall::err::{Error, Result};
use hyperion_vfs::node::{FileDriver, Ref};

//

pub static DEV_NULL: Ref<dyn FileDriver> = Ref::new_static(&Null);

//

pub struct Null;

#[async_trait]
impl FileDriver for Null {
    async fn read(
        &self,
        _: Option<&Process>,
        _: usize,
        mut buf: BufferMut<'_, u8, PageMap>,
    ) -> Result<usize> {
        unsafe {
            buf.with_slice_mut(|slice| {
                slice.fill(MaybeUninit::zeroed());
            });
        }

        Ok(buf.len())
    }

    async fn write(
        &self,
        _: Option<&Process>,
        _: usize,
        buf: Buffer<'_, u8, PageMap>,
    ) -> Result<usize> {
        Ok(buf.len())
    }
}
