use core::slice::memchr;

use core_alloc::{string::String, vec::Vec};

use crate::fs::File;

//

pub trait Read {
    fn recv(&mut self, buf: &mut [u8]) -> Result<usize, String>;
}

pub struct SimpleIpcInputChannel;

impl Read for SimpleIpcInputChannel {
    fn recv(&mut self, buf: &mut [u8]) -> Result<usize, String> {
        _ = buf;
        todo!()
        // hyperion_syscall::recv(buf).map_err(|err| format!("failed to recv: {err}"))
    }
}

impl Read for &File {
    fn recv(&mut self, buf: &mut [u8]) -> Result<usize, String> {
        self.read(buf).map_err(|e| e.as_str().into())
    }
}

pub struct BufReader<T> {
    buf: [u8; 64],
    end: u8,
    inner: T,
}

impl<T: Read> BufReader<T> {
    pub fn new(read: T) -> Self {
        Self {
            buf: [0; 64],
            end: 0,
            inner: read,
        }
    }

    pub fn read_line(&mut self, buf: &mut String) -> Result<usize, String> {
        unsafe { append_to_string(buf, |b| read_until(self, b'\n', b)) }
    }

    fn fill_buf(&mut self) -> Result<&[u8], String> {
        let bytes_read = self.inner.recv(&mut self.buf[self.end as usize..])?;
        self.end += bytes_read as u8;
        assert!((self.end as usize) <= self.buf.len());

        Ok(&self.buf[..self.end as usize])
    }

    fn consume(&mut self, used: usize) {
        self.buf.rotate_left(used);
        self.end -= used as u8;
    }
}

fn read_until<T: Read>(
    r: &mut BufReader<T>,
    delim: u8,
    buf: &mut Vec<u8>,
) -> Result<usize, String> {
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

unsafe fn append_to_string<F>(buf: &mut String, f: F) -> Result<usize, String>
where
    F: FnOnce(&mut Vec<u8>) -> Result<usize, String>,
{
    let mut g = Guard {
        len: buf.len(),
        buf: buf.as_mut_vec(),
    };
    let ret = f(g.buf);
    if core::str::from_utf8(&g.buf[g.len..]).is_err() {
        ret.and_then(|_| Err("stream did not contain valid UTF-8".into()))
    } else {
        g.len = g.buf.len();
        ret
    }
}
