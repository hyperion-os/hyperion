use alloc::{format, string::String};
use core::{fmt, ops::Deref};

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

    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> &'_ Self {
        s.as_ref()
    }

    pub fn join(&self, p: &str) -> PathBuf {
        PathBuf(format!("{}/{p}", &self.0))
    }
}

impl<'a> From<&'a str> for &'a Path {
    fn from(value: &'a str) -> Self {
        value.as_ref()
    }
}

impl fmt::Debug for Path {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Debug::fmt(&self.0, f)
    }
}

impl PathBuf {
    pub fn join(&mut self, p: &str) -> &mut PathBuf {
        use core::fmt::Write;
        _ = write!(self.0, "/{p}");
        self
    }
}

impl Deref for PathBuf {
    type Target = Path;

    fn deref(&self) -> &Self::Target {
        (self.0.as_str()).as_ref()
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

//

#[cfg(test)]
mod tests {
    use super::Path;

    #[test_case]
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
