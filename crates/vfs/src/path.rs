use core::str;

use hyperion_syscall::err::{Error, Result};

//

/// all paths handled by the VFS must be absolute, without the first `/`
/// final directories do not have the `/` at the end like `dir/`
/// because `/` is just a separator, nothing more
pub struct PathIter<'a> {
    inner: str::Split<'a, char>,
}

impl<'a> PathIter<'a> {
    pub fn new(s: &'a str) -> Self {
        Self {
            inner: s.split('/'),
        }
    }

    pub fn file_name(&self) -> Option<&'a str> {
        self.inner.clone().last()
    }
}

impl<'a> Iterator for PathIter<'a> {
    type Item = Result<(&'a str, &'a str)>;

    fn next(&mut self) -> Option<Self::Item> {
        let rem = self.inner.remainder()?;
        let part = self.inner.next()?;
        if part.is_empty() {
            return Some(Err(Error::INVALID_PATH));
        }
        Some(Ok((part, rem)))
    }
}

// impl<'a> DoubleEndedIterator for PathIter<'a> {
//     fn next_back(&mut self) -> Option<Self::Item> {
//         todo!()
//     }
// }
