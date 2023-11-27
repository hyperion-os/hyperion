use bitflags::bitflags;

//

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct FileDesc(pub usize);

//

bitflags! {
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct FileOpenFlags: usize {
    /// open file with read caps
    const READ       = 0b000001;

    /// open file with write caps
    const WRITE      = 0b000010;

    /// open file with read and write caps
    const READ_WRITE = Self::READ.bits() | Self::WRITE.bits();

    /// writes append to the file
    const APPEND     = 0b000100;

    /// create file if it doesn't already exist
    const CREATE     = 0b001000;

    /// create file if it doesn't already exist and err if it already exists
    const CREATE_NEW = 0b010000;

    /// truncate file on open (if the file already existed)
    const TRUNC      = 0b100000;
}
}
