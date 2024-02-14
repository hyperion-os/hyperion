#[cfg(not(feature = "cargo-clippy"))]
pub use std::os::hyperion::{
    net::{LocalListener, LocalStream},
    AsRawFd,
};

//

// clippy doesn't support x86_64-unknown-hyperion
#[cfg(feature = "cargo-clippy")]
pub struct LocalListener;

#[cfg(feature = "cargo-clippy")]
impl LocalListener {
    pub fn bind(_: &str) -> Result<Self, ()> {
        todo!()
    }

    pub fn accept(&self) -> Result<LocalStream, ()> {
        todo!()
    }
}

// clippy doesn't support x86_64-unknown-hyperion
#[cfg(feature = "cargo-clippy")]
pub struct LocalStream;

#[cfg(feature = "cargo-clippy")]
impl LocalStream {
    // pub fn connect(_: &str) -> Result<Self, ()> {
    //     todo!()
    // }
}

#[cfg(feature = "cargo-clippy")]
impl std::io::Read for LocalStream {
    fn read(&mut self, _: &mut [u8]) -> std::io::Result<usize> {
        todo!()
    }
}

#[cfg(feature = "cargo-clippy")]
impl BufRead for LocalStream {
    fn fill_buf(&mut self) -> std::io::Result<&[u8]> {
        todo!()
    }

    fn consume(&mut self, _: usize) {}
}

#[cfg(feature = "cargo-clippy")]
impl Write for LocalStream {
    fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
        todo!()
    }

    fn flush(&mut self) -> std::io::Result<()> {
        todo!()
    }
}

#[cfg(feature = "cargo-clippy")]
pub trait AsRawFd {
    fn as_raw_fd(&self) -> usize;
}

#[cfg(feature = "cargo-clippy")]
impl AsRawFd for File {
    fn as_raw_fd(&self) -> usize {
        todo!()
    }
}
