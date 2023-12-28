use core::mem::forget;

use hyperion_syscall::{
    accept, bind, close, connect,
    err::Result,
    fs::FileDesc,
    net::{Protocol, SocketDomain, SocketType},
    recv, send, socket,
};

use crate::{
    eprintln,
    io::{Read, Write},
};

//

pub type UnixListener = LocalListener;
pub type UnixStream = LocalStream;

//

#[derive(Debug)]
pub struct LocalListener {
    fd: FileDesc,
}

impl LocalListener {
    pub fn bind(addr: &str) -> Result<Self> {
        let fd = socket(SocketDomain::LOCAL, SocketType::STREAM, Protocol::LOCAL)?;
        bind(fd, addr)?;
        // eprintln!("LocalListener fd:{fd:?}");

        Ok(Self { fd })
    }

    /// # Safety
    ///
    /// file i/o won't be automatically synchronized,
    /// if this `fd` gets closed by a clone,
    /// this LocalListener won't know it and might use a random fd for socket syscalls
    pub unsafe fn clone(&self) -> Self {
        Self { fd: self.fd }
    }

    /// the file descriptor won't be closed automatically
    pub fn leak_fd(self) -> FileDesc {
        let fd = self.fd;
        forget(self);
        fd
    }

    pub fn accept(&self) -> Result<LocalStream> {
        let fd = accept(self.fd)?;
        // eprintln!("LocalStream (accept) fd:{fd:?}");
        Ok(LocalStream { fd })
    }

    pub fn close(self) -> Result<()> {
        // eprintln!("LocalListener close({:?})", self.fd);
        close(self.leak_fd())?;
        Ok(())
    }
}

impl Drop for LocalListener {
    fn drop(&mut self) {
        // a hacky ManuallyDrop
        unsafe { self.clone() }.close().unwrap();
    }
}

//

#[derive(Debug)]
pub struct LocalStream {
    fd: FileDesc,
}

impl LocalStream {
    pub fn connect(addr: &str) -> Result<Self> {
        let fd = socket(SocketDomain::LOCAL, SocketType::STREAM, Protocol::LOCAL)?;
        connect(fd, addr)?;
        // eprintln!("LocalStream (connect) fd:{fd:?}");

        Ok(Self { fd })
    }

    /// # Safety
    ///
    /// file i/o won't be automatically synchronized,
    /// if this `fd` gets closed by a clone,
    /// this LocalListener won't know it and might use a random fd for socket syscalls
    pub unsafe fn clone(&self) -> Self {
        Self { fd: self.fd }
    }

    /// the file descriptor won't be closed automatically
    pub fn leak_fd(self) -> FileDesc {
        let fd = self.fd;
        forget(self);
        fd
    }

    pub fn close(self) -> Result<()> {
        // eprintln!("LocalStream close({:?})", self.fd);
        close(self.leak_fd())?;
        Ok(())
    }
}

impl Read for LocalStream {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
        // eprintln!("recv({:?}, [ .. ], 0)", self.fd);
        recv(self.fd, buf, 0)
    }
}

impl Write for LocalStream {
    fn write(&mut self, buf: &[u8]) -> Result<usize> {
        // eprintln!("send({:?}, [ .. ], 0)", self.fd);
        send(self.fd, buf, 0)
    }

    fn flush(&mut self) -> Result<()> {
        Ok(())
    }
}

impl Drop for LocalStream {
    fn drop(&mut self) {
        unsafe { self.clone() }.close().unwrap();
    }
}
