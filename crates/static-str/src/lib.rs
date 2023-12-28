#![no_std]

//

use core::{fmt, ops::Deref, str::Utf8Error};

// a string that doesn't require allocations
//
// invariant:
//  - self.bytes is always valid utf8
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct StaticStr<const SIZE: usize> {
    bytes: [u8; SIZE],
}

//

impl<const SIZE: usize> StaticStr<SIZE> {
    pub fn from_utf8(bytes: [u8; SIZE]) -> Result<Self, Utf8Error> {
        _ = core::str::from_utf8(zero_limited(&bytes))?;
        Ok(unsafe { Self::from_utf8_unchecked(bytes) })
    }

    /// # Safety
    ///
    /// `bytes` must contain valid utf8 until the first zero byte
    #[must_use]
    pub const unsafe fn from_utf8_unchecked(bytes: [u8; SIZE]) -> Self {
        Self { bytes }
    }

    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        zero_limited(&self.bytes)
    }

    pub fn as_str_checked(&self) -> Result<&str, Utf8Error> {
        core::str::from_utf8(zero_limited(&self.bytes))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        unsafe { core::str::from_utf8_unchecked(zero_limited(&self.bytes)) }
    }
}

impl<const SIZE: usize> Deref for StaticStr<SIZE> {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        self.as_str()
    }
}

impl<const SIZE: usize> fmt::Debug for StaticStr<SIZE> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Debug::fmt(self.as_str(), f)
    }
}

impl<const SIZE: usize> fmt::Display for StaticStr<SIZE> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Display::fmt(self.as_str(), f)
    }
}

fn zero_limited(bytes: &[u8]) -> &[u8] {
    let first_zero = bytes
        .iter()
        .enumerate()
        .find(|(_, b)| **b == 0)
        .map_or(bytes.len(), |(i, _)| i);

    &bytes[..first_zero]
}
