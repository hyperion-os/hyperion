use alloc::{
    borrow::{Cow, ToOwned},
    format,
    string::{String, ToString},
};
use core::{
    borrow::Borrow,
    fmt::{self, Write},
    ops::Deref,
};

//

pub struct Path(pub str);

#[derive(Clone, PartialEq, Eq)]
pub struct PathBuf(pub String);

//

impl Path {
    pub fn parent(&self) -> Option<&Path> {
        self.split().map(|(parent, _)| parent)
    }

    pub fn file_name(&self) -> Option<&str> {
        self.split().map(|(_, file)| file)
    }

    pub fn split(&self) -> Option<(&Path, &str)> {
        self.0
            .rsplit_once('/')
            .map(|(parent, file)| (parent.as_ref(), file))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn iter(&self) -> impl Iterator<Item = &str> {
        self.0
            .trim_matches('/')
            .split('/')
            .filter(|p| !p.is_empty())
    }

    pub fn is_dir(&self) -> bool {
        self.0.ends_with('/')
    }

    pub fn is_file(&self) -> bool {
        !self.is_dir()
    }

    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> &'_ Self {
        s.as_ref()
    }

    pub fn join(&self, p: impl AsRef<Path>) -> PathBuf {
        if self.is_dir() {
            PathBuf(format!("{}{}", self.as_str(), p.as_ref().as_str()))
        } else {
            PathBuf(format!("{}/{}", self.as_str(), p.as_ref().as_str()))
        }
    }

    pub fn is_absolute(&self) -> bool {
        self.0.starts_with('/')
    }

    pub fn to_absolute(&self, working_dir: &Path) -> Cow<'_, Path> {
        if self.is_absolute() {
            Cow::Borrowed(self)
        } else {
            let mut working_dir = working_dir.to_owned();

            // relative path
            for part in self.iter() {
                match part {
                    "." => {}
                    ".." => {
                        working_dir.pop();
                    }
                    other => {
                        working_dir.join(other);
                    }
                }
            }

            Cow::Owned(working_dir)
        }
    }
}

impl<'a> From<&'a str> for &'a Path {
    fn from(value: &'a str) -> Self {
        value.as_ref()
    }
}

impl AsRef<Path> for str {
    fn as_ref(&self) -> &Path {
        unsafe { &*(self as *const str as *const Path) }
    }
}

impl AsRef<Path> for Path {
    fn as_ref(&self) -> &Path {
        self
    }
}

impl fmt::Debug for Path {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Debug::fmt(&self.0, f)
    }
}

impl PathBuf {
    pub fn new(p: impl AsRef<Path>) -> Self {
        Self(p.as_ref().as_str().to_string())
    }

    pub fn set(&mut self, p: impl AsRef<Path>) -> &mut PathBuf {
        self.0.clear();
        _ = write!(self.0, "{}", p.as_ref().as_str());
        self
    }

    pub fn pop(&mut self) -> &mut PathBuf {
        if let Some(split) = self.0.rfind('/') {
            self.0.truncate(split + 1);
        }
        self
    }

    pub fn join(&mut self, p: &str) -> &mut PathBuf {
        if self.is_dir() {
            _ = write!(self.0, "{p}");
        } else {
            _ = write!(self.0, "/{p}");
        }
        self
    }
}

impl Deref for PathBuf {
    type Target = Path;

    fn deref(&self) -> &Self::Target {
        self.borrow()
    }
}

impl fmt::Debug for PathBuf {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Debug::fmt(&self.0, f)
    }
}

impl Borrow<Path> for PathBuf {
    fn borrow(&self) -> &Path {
        (self.0.as_str()).as_ref()
    }
}

impl ToOwned for Path {
    type Owned = PathBuf;

    fn to_owned(&self) -> Self::Owned {
        PathBuf(self.0.to_owned())
    }
}

impl AsRef<Path> for PathBuf {
    fn as_ref(&self) -> &Path {
        self.borrow()
    }
}

impl From<&Path> for PathBuf {
    fn from(value: &Path) -> Self {
        Self::new(value)
    }
}

impl From<&Path> for Option<PathBuf> {
    fn from(val: &Path) -> Self {
        Some(val.into())
    }
}

//

#[cfg(test)]
mod tests {
    use super::Path;

    #[test]
    fn path_unsafe_test_2() {
        let path = "/some/path";
        let path: &Path = path.as_ref();

        let mut parts = path.iter();

        assert_eq!(parts.next(), Some("/"));
        assert_eq!(parts.next(), Some("some"));
        assert_eq!(parts.next(), Some("path"));
        assert_eq!(parts.next(), None);
    }
}
