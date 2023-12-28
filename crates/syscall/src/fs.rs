use bitflags::bitflags;

//

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Seek(pub usize);

impl Seek {
    pub const SET: Self = Seek(0);
    pub const CUR: Self = Seek(1);
    pub const END: Self = Seek(2);
}

//

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct FileDesc(pub usize);

//

bitflags! {
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct FileOpenFlags: usize {
    /// open file with read caps
    const READ       = 0b00_0001;

    /// open file with write caps
    const WRITE      = 0b00_0010;

    /// open file with read and write caps
    const READ_WRITE = Self::READ.bits() | Self::WRITE.bits();

    /// writes append to the file
    const APPEND     = 0b00_0100;

    /// create file if it doesn't already exist
    const CREATE     = 0b00_1000;

    /// create file if it doesn't already exist and err if it already exists
    const CREATE_NEW = 0b01_0000;

    /// truncate file on open (if the file already existed)
    const TRUNC      = 0b10_0000;
}
}

//

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Metadata {
    pub len: usize,
    pub position: usize,
}

impl Metadata {
    #[must_use]
    pub const fn zeroed() -> Self {
        Self {
            len: 0,
            position: 0,
        }
    }

    #[must_use]
    pub const fn len(&self) -> usize {
        self.len
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    #[must_use]
    pub fn position(&self) -> usize {
        self.position
    }
}
