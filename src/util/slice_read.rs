use crate::vfs::{FileDevice, IoError, IoResult};

//

impl FileDevice for &'_ [u8] {
    fn len(&self) -> usize {
        self.len()
    }

    fn read(&self, offset: usize, buf: &mut [u8]) -> IoResult<usize> {
        let len = self
            .len()
            .checked_sub(offset)
            .ok_or(IoError::UnexpectedEOF)?
            .min(buf.len());

        buf[..len].copy_from_slice(&self[offset..offset + len]);

        Ok(len)
    }

    fn write(&mut self, offset: usize, buf: &[u8]) -> IoResult<usize> {
        Err(IoError::PermissionDenied)
    }
}

impl FileDevice for &'_ mut [u8] {
    fn len(&self) -> usize {
        self.len()
    }

    fn read(&self, offset: usize, buf: &mut [u8]) -> IoResult<usize> {
        self.as_ref().read(offset, buf)
    }

    fn write(&mut self, offset: usize, buf: &[u8]) -> IoResult<usize> {
        let len = self
            .len()
            .checked_sub(offset)
            .ok_or(IoError::UnexpectedEOF)?
            .min(buf.len());

        self[offset..offset + len].copy_from_slice(&buf[..len]);

        Ok(len)
    }
}
