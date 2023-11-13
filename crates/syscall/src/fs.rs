use crate::{close, err::Result, open, read, write};

//

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct FileDesc(pub usize);

//

pub struct File {
    desc: FileDesc,
}

impl File {
    pub fn open(path: &str) -> Result<Self> {
        let desc = open(path, 0, 0)?;
        Ok(Self { desc })
    }

    pub fn read(&self, buf: &mut [u8]) -> Result<usize> {
        read(self.desc, buf)
    }

    pub fn write(&self, buf: &[u8]) -> Result<usize> {
        write(self.desc, buf)
    }

    pub fn close(&self) -> Result<()> {
        close(self.desc)
    }
}

impl Drop for File {
    fn drop(&mut self) {
        self.close().expect("failed to close the file");
    }
}
