#![no_std]

//

use hyperion_vfs::{IoError, IoResult};

//

pub fn slice_read(this: &[u8], offset: usize, buf: &mut [u8]) -> IoResult<usize> {
    let len = this
        .len()
        .checked_sub(offset)
        .ok_or(IoError::UnexpectedEOF)?
        .min(buf.len());

    buf[..len].copy_from_slice(&this[offset..offset + len]);

    Ok(len)
}

pub fn slice_write(this: &mut [u8], offset: usize, buf: &[u8]) -> IoResult<usize> {
    let len = this
        .len()
        .checked_sub(offset)
        .ok_or(IoError::UnexpectedEOF)?
        .min(buf.len());

    this[offset..offset + len].copy_from_slice(&buf[..len]);

    Ok(len)
}
