use core::{fmt, mem};

use core_alloc::{boxed::Box, string::String, vec::Vec};
use hyperion_syscall::err::{Error, Result};
pub use stdio::*;

//

mod stdio;

//

pub trait Read {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize>;

    // fn read_to_end(&mut self, buf: &mut Vec<u8>) -> Result<usize> {
    //     std::io::Read::read_to_end;
    //     default_read_to_end(self, buf, None)
    // }

    // fn read_to_string(&mut self, buf: &mut String) -> Result<usize> {
    //     default_read_to_string(self, buf, None)
    // }

    fn read_exact(&mut self, mut buf: &mut [u8]) -> Result<()> {
        while !buf.is_empty() {
            match self.read(buf) {
                Ok(0) => break,
                Ok(n) => {
                    let tmp = buf;
                    buf = &mut tmp[n..];
                }
                Err(Error::INTERRUPTED) => {}
                Err(err) => return Err(err),
            }
        }

        if !buf.is_empty() {
            Err(Error::UNEXPECTED_EOF)
        } else {
            Ok(())
        }
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

    fn write_all(&mut self, mut buf: &[u8]) -> Result<()> {
        while !buf.is_empty() {
            match self.write(buf) {
                Ok(0) => return Err(Error::WRITE_ZERO),
                Ok(n) => buf = &buf[n..],
                Err(Error::INTERRUPTED) => {}
                Err(err) => return Err(err),
            }
        }
        Ok(())
    }

    fn flush(&mut self) -> Result<()>;

    fn write_fmt(&mut self, args: fmt::Arguments) -> Result<()> {
        struct FmtWrite<'a, T: ?Sized> {
            inner: &'a mut T,
            err: Result<()>,
        }

        impl<T: ?Sized + Write> fmt::Write for FmtWrite<'_, T> {
            fn write_str(&mut self, s: &str) -> fmt::Result {
                self.err = self.inner.write_all(s.as_bytes());

                if self.err.is_err() {
                    Err(fmt::Error)
                } else {
                    Ok(())
                }
            }
        }

        let mut writer = FmtWrite {
            inner: self,
            err: Ok(()),
        };
        _ = fmt::write(&mut writer, args);
        writer.err?;
        writer.inner.flush()?;
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

//

pub trait WriteExt {
    fn fmt(&mut self) -> &mut FmtWrite<Self>;
}

impl<T: Write> WriteExt for T {
    fn fmt(&mut self) -> &mut FmtWrite<Self> {
        unsafe { mem::transmute::<&mut T, &mut FmtWrite<T>>(self) }
    }
}

//

#[repr(transparent)]
pub struct FmtWrite<T: ?Sized>(T);

impl<T: ?Sized + Write> fmt::Write for FmtWrite<T> {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        if self.0.write_all(s.as_bytes()).is_err() {
            Err(fmt::Error)
        } else {
            Ok(())
        }
    }

    fn write_fmt(&mut self, args: fmt::Arguments) -> fmt::Result {
        if Write::write_fmt(&mut self.0, args).is_err() {
            Err(fmt::Error)
        } else {
            Ok(())
        }
    }
}

//

pub struct BufReader<T> {
    buf: Option<Box<[u8]>>,
    end: usize,
    inner: T,
}

impl<T: Read> BufReader<T> {
    pub const fn new(read: T) -> Self {
        Self {
            buf: None,
            end: 0,
            inner: read,
        }
    }

    pub fn get_ref(&self) -> &T {
        &self.inner
    }

    pub fn get_mut(&mut self) -> &mut T {
        &mut self.inner
    }

    pub fn into_inner(self) -> T {
        self.inner
    }

    fn buf(buf: &mut Option<Box<[u8]>>) -> &mut [u8] {
        buf.get_or_insert_with(|| unsafe { Box::new_zeroed_slice(0x4000).assume_init() })
    }

    pub fn read_line(&mut self, buf: &mut String) -> Result<usize> {
        unsafe { append_to_string(buf, |b| read_until(self, b'\n', b)) }
    }

    pub fn fill_buf(&mut self) -> Result<&[u8]> {
        let buf = Self::buf(&mut self.buf);

        if self.end != 0 {
            return Ok(&buf[..self.end]);
        }

        let bytes_read = self.inner.read(&mut buf[self.end..])?;
        self.end += bytes_read;
        assert!(self.end <= buf.len());

        Ok(&buf[..self.end])
    }

    pub fn consume(&mut self, used: usize) {
        let buf = Self::buf(&mut self.buf);
        buf.rotate_left(used);
        self.end -= used;
    }
}

// impl<T: Read> Read for BufReader<T> {
//     fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
//         todo!()
//     }
// }

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

    pub fn get_ref(&self) -> &T {
        &self.inner
    }

    pub fn get_mut(&mut self) -> &mut T {
        &mut self.inner
    }
}

impl<T: Write> Write for BufWriter<T> {
    fn write(&mut self, buf: &[u8]) -> Result<usize> {
        assert!(!self.broken, "BufWriter broken");

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

// TODO: BufReader and ConstBufReader should impl BufRead instead
fn read_until<T: Read>(r: &mut BufReader<T>, delim: u8, buf: &mut Vec<u8>) -> Result<usize> {
    let mut read = 0;
    loop {
        let (done, used) = {
            let available = r.fill_buf()?;
            match available.iter().position(|&c| c == delim) {
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
        buf: unsafe { buf.as_mut_vec() },
    };
    let ret = f(g.buf);
    if core::str::from_utf8(&g.buf[g.len..]).is_err() {
        ret.and(Err(Error::INVALID_UTF8))
    } else {
        g.len = g.buf.len();
        ret
    }
}
