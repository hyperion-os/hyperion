use hyperion_driver_acpi::hpet::HPET;
use hyperion_vfs::{
    device::FileDevice,
    error::{IoError, IoResult},
};

//

pub struct HpetDevice;

//

impl FileDevice for HpetDevice {
    fn as_any(&self) -> &dyn core::any::Any {
        self
    }

    fn len(&self) -> usize {
        core::mem::size_of::<i64>()
    }

    fn set_len(&mut self, _: usize) -> IoResult<()> {
        Err(IoError::PermissionDenied)
    }

    fn read(&self, offset: usize, buf: &mut [u8]) -> IoResult<usize> {
        let bytes = &HPET.now_bytes()[..];
        bytes.read(offset, buf)
    }

    fn write(&mut self, _: usize, _: &[u8]) -> IoResult<usize> {
        Err(IoError::PermissionDenied)
    }
}
