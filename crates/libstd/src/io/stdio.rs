use core::ops::{Deref, DerefMut};

use hyperion_syscall::{err::Result, fs::FileDesc, read, write};
use spin::Once;

use super::{Read, Write};
use crate::{
    io::{BufReader, BufWriter},
    sync::{Mutex, MutexGuard},
};

//

pub fn stdin() -> Stdin {
    static STDIN: Once<Mutex<BufReader<Stdio<0>>>> = Once::new(); // TODO: userspace Once + Lazy

    Stdin {
        inner: STDIN.call_once(|| Mutex::new(BufReader::new(Stdio))),
    }
}

pub fn stdout() -> Stdout {
    static STDOUT: Once<Mutex<BufWriter<Stdio<1>>>> = Once::new();

    Stdout {
        inner: STDOUT.call_once(|| Mutex::new(BufWriter::new(Stdio))),
    }
}

pub fn stderr() -> Stderr {
    static STDERR: Once<Mutex<BufWriter<Stdio<2>>>> = Once::new();

    Stderr {
        inner: STDERR.call_once(|| Mutex::new(BufWriter::new(Stdio))),
    }
}

//

pub struct Stdin {
    inner: &'static Mutex<BufReader<Stdio<0>>>,
}

impl Stdin {
    pub const FD: FileDesc = FileDesc(0);

    pub fn lock(&self) -> StdinLock {
        StdinLock {
            inner: self.inner.lock(),
        }
    }
}

//

pub struct StdinLock {
    inner: MutexGuard<'static, BufReader<Stdio<0>>>,
}

// impl Read for StdinLock {
//     fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
//         todo!()
//     }
// }

impl Deref for StdinLock {
    type Target = BufReader<Stdio<0>>;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl DerefMut for StdinLock {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}

//

pub struct Stdout {
    inner: &'static Mutex<BufWriter<Stdio<1>>>,
}

impl Stdout {
    pub const FD: FileDesc = FileDesc(1);

    pub fn lock(&self) -> StdoutLock {
        StdoutLock {
            inner: self.inner.lock(),
        }
    }
}

pub struct StdoutLock {
    inner: MutexGuard<'static, BufWriter<Stdio<1>>>,
}

impl Deref for StdoutLock {
    type Target = BufWriter<Stdio<1>>;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl DerefMut for StdoutLock {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}

//

pub struct Stderr {
    inner: &'static Mutex<BufWriter<Stdio<2>>>,
}

impl Stderr {
    pub const FD: FileDesc = FileDesc(2);

    pub fn lock(&self) -> StderrLock {
        StderrLock {
            inner: self.inner.lock(),
        }
    }
}

pub struct StderrLock {
    inner: MutexGuard<'static, BufWriter<Stdio<2>>>,
}

impl Deref for StderrLock {
    type Target = BufWriter<Stdio<2>>;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl DerefMut for StderrLock {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}

//

pub struct Stdio<const FD: u8>;

impl<const FD: u8> Read for Stdio<FD> {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
        read(FileDesc(FD as _), buf)
    }
}

impl<const FD: u8> Write for Stdio<FD> {
    fn write(&mut self, buf: &[u8]) -> Result<usize> {
        write(FileDesc(FD as _), buf)
    }

    fn flush(&mut self) -> Result<()> {
        Ok(())
    }
}
