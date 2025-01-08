use alloc::{boxed::Box, collections::btree_map::BTreeMap, sync::Arc};
use core::{fmt, ops::Deref, ptr::NonNull};

use async_trait::async_trait;
use hyperion_arch::vmm::PageMap;
use hyperion_futures::lock::Mutex;
use hyperion_mem::buf::{Buffer, BufferMut};
use hyperion_scheduler::proc::Process;
use hyperion_syscall::err::{Error, Result};

//

#[derive(Clone)]
pub enum Node {
    File(Ref<FileNode>),
    Dir(Ref<DirNode>),
}

impl Node {
    pub const fn as_dir(&self) -> Option<&Ref<DirNode>> {
        match self {
            Node::Dir(dir_node) => Some(dir_node),
            _ => None,
        }
    }

    pub fn to_dir(self) -> Option<Ref<DirNode>> {
        match self {
            Node::Dir(dir_node) => Some(dir_node),
            _ => None,
        }
    }

    pub const fn as_file(&self) -> Option<&Ref<FileNode>> {
        match self {
            Node::File(file_node) => Some(file_node),
            _ => None,
        }
    }

    pub fn to_file(self) -> Option<Ref<FileNode>> {
        match self {
            Node::File(file_node) => Some(file_node),
            _ => None,
        }
    }
}

//

pub struct FileNode {
    // all normal inode info here: create date, modify date, ...

    // TODO: maybe a link to the driver and then some inode ID,
    // instead of every file having its own allocation
    pub driver: Mutex<Ref<dyn FileDriver>>,
}

impl FileNode {}

#[async_trait]
pub trait FileDriver: Send + Sync {
    async fn read(
        &self,
        proc: Option<&Process>,
        offset: usize,
        buf: BufferMut<'_, u8, PageMap>,
    ) -> Result<usize> {
        _ = (proc, buf);
        Err(Error::PERMISSION_DENIED)
    }

    async fn write(
        &self,
        proc: Option<&Process>,
        offset: usize,
        buf: Buffer<'_, u8, PageMap>,
    ) -> Result<usize> {
        _ = (proc, buf);
        Err(Error::PERMISSION_DENIED)
    }
}

//

pub struct DirNode {
    pub nodes: Mutex<BTreeMap<Arc<str>, Node>>,

    // TODO: same todo as `FileNode::driver`
    pub driver: Mutex<Ref<dyn DirDriver>>,
}

#[async_trait]
pub trait DirDriver: Send + Sync {
    /// get a sub-directory or file in this directory as a cache Node
    async fn get(&self, proc: Option<&Process>, name: &str) -> Result<(Node, CacheAllowed)> {
        _ = (proc, name);
        Err(Error::NOT_FOUND)
    }

    /// create a new sub-directory in this directory and return a cache Node
    async fn create_dir(&self, proc: Option<&Process>, name: &str) -> Result<(Node, CacheAllowed)> {
        _ = (proc, name);
        Err(Error::PERMISSION_DENIED)
    }

    /// create a new file in this directory and return a cache Node
    async fn create_file(
        &self,
        proc: Option<&Process>,
        name: &str,
    ) -> Result<(Node, CacheAllowed)> {
        _ = (proc, name);
        Err(Error::PERMISSION_DENIED)
    }
}

//

pub type CacheAllowed = bool;

//

pub struct Ref<T: ?Sized, U: ?Sized = T> {
    ptr: NonNull<T>,
    arc: Option<Arc<U>>,
}

unsafe impl<T: ?Sized + Sync + Send, U: ?Sized + Sync + Send> Send for Ref<T, U> {}
unsafe impl<T: ?Sized + Sync + Send, U: ?Sized + Sync + Send> Sync for Ref<T, U> {}

// impl<S: ?Sized, T: ?Sized + Unsize<U>, U: ?Sized> CoerceUnsized<Ref<S, U>> for Ref<S, T> {}
// impl<S: ?Sized, T: ?Sized + Unsize<U>, U: ?Sized> CoerceUnsized<Ref<U, S>> for Ref<T, S> {}

impl<T: ?Sized, U: ?Sized> Ref<T, U> {
    pub fn map<V, F: for<'a> FnOnce(&'a T) -> &'a V>(self, f: F) -> Ref<V, U> {
        let new_ptr = f(self.deref());
        // SAFETY: a ref cannot be null, the lifetime is the same as the original
        let ptr = unsafe { NonNull::new_unchecked(new_ptr as *const _ as _) };
        Ref { ptr, arc: self.arc }
    }
}

impl<T> Ref<T, T> {
    pub fn new(val: T) -> Self {
        Self::from_arc(Arc::new(val))
    }
}

impl<T: ?Sized> Ref<T, T> {
    pub const fn new_static(val: &'static T) -> Self {
        // SAFETY: a ref cannot be null
        let ptr = unsafe { NonNull::new_unchecked(val as *const _ as _) };
        Self { ptr, arc: None }
    }

    pub fn from_arc(val: Arc<T>) -> Self {
        // SAFETY: a ref cannot be null, `ptr` is valid as long as `arc` is
        let ptr = unsafe { NonNull::new_unchecked(Arc::as_ptr(&val) as _) };
        let arc = Some(val);
        Self { ptr, arc }
    }
}

impl<T: ?Sized, U: ?Sized> Deref for Ref<T, U> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        // SAFETY: the pointer is either to a &'static T, or the data inside an Arc that is currently held
        unsafe { self.ptr.as_ref() }
    }
}

impl<T: ?Sized, U: ?Sized> Clone for Ref<T, U> {
    fn clone(&self) -> Self {
        Self {
            ptr: self.ptr,
            arc: self.arc.clone(),
        }
    }
}

impl<T: ?Sized + fmt::Debug, U: ?Sized> fmt::Debug for Ref<T, U> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.deref().fmt(f)
    }
}
