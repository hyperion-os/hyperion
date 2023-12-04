use core::{fmt, slice::memchr};

use core_alloc::{boxed::Box, string::String, vec::Vec};
use hyperion_syscall::err::{Error, Result};

use crate::fs::File;

//

pub trait Read {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize>;
}

impl Read for File {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
        File::read(self, buf)
    }
}

impl<T: Read> Read for &mut T {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
        (**self).read(buf)
    }
}

//

pub trait Write {
    fn write(&mut self, buf: &[u8]) -> Result<usize>;

    fn flush(&mut self) -> Result<()>;
}

impl Write for File {
    fn write(&mut self, buf: &[u8]) -> Result<usize> {
        File::write(self, buf)
    }

    fn flush(&mut self) -> Result<()> {
        Ok(())
    }
}

impl<T: Write> Write for &mut T {
    fn write(&mut self, buf: &[u8]) -> Result<usize> {
        (**self).write(buf)
    }

    fn flush(&mut self) -> Result<()> {
        (**self).flush()
    }
}

impl fmt::Write for File {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        self.write(s.as_bytes()).map_err(|_| fmt::Error)?;
        Ok(())
    }
}

impl<T: Write> fmt::Write for BufWriter<T> {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        self.write(s.as_bytes()).map_err(|_| fmt::Error)?;
        Ok(())
    }

    fn write_fmt(mut self: &mut Self, args: fmt::Arguments) -> fmt::Result {
        fmt::write(&mut self, args)?;
        self.flush().map_err(|_| fmt::Error)?;
        Ok(())
    }
}

//

pub struct BufReader<T> {
    buf: Box<[u8]>,
    end: u8,
    inner: T,
}

impl<T: Read> BufReader<T> {
    pub fn new(read: T) -> Self {
        Self {
            buf: unsafe { Box::new_zeroed_slice(512).assume_init() },
            end: 0,
            inner: read,
        }
    }

    pub fn read_line(&mut self, buf: &mut String) -> Result<usize> {
        unsafe { append_to_string(buf, |b| read_until(self, b'\n', b)) }
    }

    fn fill_buf(&mut self) -> Result<&[u8]> {
        let bytes_read = self.inner.read(&mut self.buf[self.end as usize..])?;
        self.end += bytes_read as u8;
        assert!((self.end as usize) <= self.buf.len());

        Ok(&self.buf[..self.end as usize])
    }

    fn consume(&mut self, used: usize) {
        self.buf.rotate_left(used);
        self.end -= used as u8;
    }
}

pub struct ConstBufReader<'a, T> {
    buf: &'a mut [u8],
    end: u8,
    inner: T,
}

impl<'a, T: Read> ConstBufReader<'a, T> {
    pub const fn new(read: T, buf: &'a mut [u8]) -> Self {
        Self {
            buf,
            end: 0,
            inner: read,
        }
    }

    pub fn read_line(&mut self, buf: &mut String) -> Result<usize> {
        unsafe { append_to_string(buf, |b| read_until_2(self, b'\n', b)) }
    }

    fn fill_buf(&mut self) -> Result<&[u8]> {
        let bytes_read = self.inner.read(&mut self.buf[self.end as usize..])?;
        self.end += bytes_read as u8;
        assert!((self.end as usize) <= self.buf.len());

        Ok(&self.buf[..self.end as usize])
    }

    fn consume(&mut self, used: usize) {
        self.buf.rotate_left(used);
        self.end -= used as u8;
    }
}

//

pub struct BufWriter<T> {
    buf: Vec<u8>,
    broken: bool,
    inner: T,
}

impl<T> BufWriter<T> {
    pub const fn new(write: T) -> Self {
        Self {
            buf: Vec::new(),
            broken: false,
            inner: write,
        }
    }
}

impl<T: Write> Write for BufWriter<T> {
    fn write(&mut self, buf: &[u8]) -> Result<usize> {
        if self.broken {
            panic!("BufWriter broken");
        }

        self.buf.extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> Result<()> {
        let mut consumed = 0usize;

        while consumed < self.buf.len() {
            match self.inner.write(&self.buf) {
                Ok(b) => consumed += b,
                Err(Error::INTERRUPTED) => {}
                Err(err) => {
                    self.broken = true;
                    return Err(err);
                }
            }
        }

        self.buf.clear();
        Ok(())
    }
}

//

fn read_until<T: Read>(r: &mut BufReader<T>, delim: u8, buf: &mut Vec<u8>) -> Result<usize> {
    let mut read = 0;
    loop {
        let (done, used) = {
            let available = r.fill_buf()?;
            match memchr::memchr(delim, available) {
                Some(i) => {
                    buf.extend_from_slice(&available[..=i]);
                    (true, i + 1)
                }
                None => {
                    buf.extend_from_slice(available);
                    (false, available.len())
                }
            }
        };
        r.consume(used);
        read += used;
        if done || used == 0 {
            return Ok(read);
        }
    }
}

fn read_until_2<T: Read>(r: &mut ConstBufReader<T>, delim: u8, buf: &mut Vec<u8>) -> Result<usize> {
    let mut read = 0;
    loop {
        let (done, used) = {
            let available = r.fill_buf()?;
            match memchr::memchr(delim, available) {
                Some(i) => {
                    buf.extend_from_slice(&available[..=i]);
                    (true, i + 1)
                }
                None => {
                    buf.extend_from_slice(available);
                    (false, available.len())
                }
            }
        };
        r.consume(used);
        read += used;
        if done || used == 0 {
            return Ok(read);
        }
    }
}

struct Guard<'a> {
    buf: &'a mut Vec<u8>,
    len: usize,
}

impl Drop for Guard<'_> {
    fn drop(&mut self) {
        unsafe {
            self.buf.set_len(self.len);
        }
    }
}

unsafe fn append_to_string<F>(buf: &mut String, f: F) -> Result<usize>
where
    F: FnOnce(&mut Vec<u8>) -> Result<usize>,
{
    let mut g = Guard {
        len: buf.len(),
        buf: buf.as_mut_vec(),
    };
    let ret = f(g.buf);
    if core::str::from_utf8(&g.buf[g.len..]).is_err() {
        ret.and(Err(Error::INVALID_UTF8))
    } else {
        g.len = g.buf.len();
        ret
    }
}
