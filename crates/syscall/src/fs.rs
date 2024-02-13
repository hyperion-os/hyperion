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
    const READ       = 0b0000_0001;

    /// open file with write caps
    const WRITE      = 0b0000_0010;

    /// open file with read and write caps
    const READ_WRITE = Self::READ.bits() | Self::WRITE.bits();

    /// writes append to the file
    const APPEND     = 0b0000_0100;

    /// create file if it doesn't already exist
    const CREATE     = 0b0000_1000;

    /// create file if it doesn't already exist and err if it already exists
    const CREATE_NEW = 0b0001_0000;

    /// truncate file on open (if the file already existed)
    const TRUNC      = 0b0010_0000;

    /// the opened file is actually a directory, a directory is a virtual file of its contents:
    /// <item-name> <item-size> <item-mode>
    const IS_DIR     = 0b0100_0000;

    /// create all parent directories
    const CREATE_DIRS= 0b1000_0000;
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
