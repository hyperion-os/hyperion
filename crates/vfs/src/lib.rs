#![no_std]
#![feature(
    str_split_remainder,
    let_chains,
    stmt_expr_attributes,
    coerce_unsized,
    unsize,
    future_join,
    map_try_insert
)]

use core::future::join;

use hyperion_futures::lock::Mutex;
use hyperion_syscall::err::{Error, Result};
use spin::Once;

use self::node::{DirDriver, DirNode, FileDriver, FileNode, Node, Ref};

//

extern crate alloc;

//

pub mod node;
pub mod path;
pub mod tmpfs;

//

static ROOT: Once<Ref<DirNode>> = Once::new();

//

#[derive(Debug, Clone, Copy)]
pub struct OpenOptions {
    existing: ExistingPolicy,
    missing: MissingPolicy,
}

impl OpenOptions {
    pub const fn new() -> Self {
        Self {
            existing: ExistingPolicy::UseExisting,
            missing: MissingPolicy::Error,
        }
    }
}

//

#[derive(Debug, Clone, Copy)]
pub enum ExistingPolicy {
    UseExisting,
    Error,
}

#[derive(Debug, Clone, Copy)]
pub enum MissingPolicy {
    CreateDir,
    CreateFile,
    Error,
}

impl MissingPolicy {
    pub const fn is_readonly(&self) -> bool {
        match self {
            MissingPolicy::CreateDir | MissingPolicy::CreateFile => false,
            MissingPolicy::Error => true,
        }
    }
}

//

pub async fn get_file(path: &str, opts: OpenOptions) -> Result<Ref<FileNode>> {
    get(path, opts).await?.to_file().ok_or(Error::NOT_A_FILE)
}

pub async fn get_dir(path: &str, opts: OpenOptions) -> Result<Ref<DirNode>> {
    get(path, opts)
        .await?
        .to_dir()
        .ok_or(Error::NOT_A_DIRECTORY)
}

pub async fn mount(path: &str, dev: Ref<dyn DirDriver>) -> Result<Ref<DirNode>> {
    let node = get_dir(
        path,
        OpenOptions {
            existing: ExistingPolicy::UseExisting,
            missing: MissingPolicy::CreateDir,
        },
    )
    .await?;

    let (mut nodes, mut driver) = join!(node.nodes.lock(), node.driver.lock()).await;

    nodes.clear();
    *driver = dev;

    drop((nodes, driver));

    Ok(node)
}

pub async fn unmount(path: &str) -> Result<()> {
    let (parent_dir, name) = path.rsplit_once('/').unwrap_or(("", path));
    let parent = get_dir(
        parent_dir,
        OpenOptions {
            existing: ExistingPolicy::UseExisting,
            missing: MissingPolicy::Error,
        },
    )
    .await?;

    // FIXME: currently unmounts files and non mounted directories
    parent
        .nodes
        .lock()
        .await
        .remove(name)
        .ok_or(Error::NOT_FOUND)?;

    Ok(())
}

pub async fn bind(path: &str, dev: Ref<dyn FileDriver>) -> Result<Ref<FileNode>> {
    let (parent_dir, name) = path.rsplit_once('/').unwrap_or(("", path));
    let parent = get_dir(
        parent_dir,
        OpenOptions {
            existing: ExistingPolicy::UseExisting,
            missing: MissingPolicy::Error,
        },
    )
    .await?;

    let node = Ref::new(FileNode {
        driver: Mutex::new(dev),
    });

    parent
        .nodes
        .lock()
        .await
        .try_insert(name.into(), Node::File(node.clone()))
        .map_err(|_| Error::ALREADY_EXISTS)?;

    Ok(node)
}

pub async fn unbind(path: &str) -> Result<()> {
    unmount(path).await
}

/// travel through the node graph and try to find the file/dir at `path`
///
/// if `create_dirs` is set,
/// then it creates directories every time it cannot find a node
/// (except on the root because missing root means there is no driver)
pub async fn get(path: &str, opts: OpenOptions) -> Result<Node> {
    let mut cur = Node::Dir(ROOT.get().ok_or(Error::NOT_FOUND)?.clone());

    for (part, _part_to_end) in path::PathIter::new(path) {
        let cur_dir = cur.to_dir().ok_or(Error::NOT_A_DIRECTORY)?;

        let is_last = part != _part_to_end;
        let (existing, missing) = if is_last {
            // use the provided open options for the final file/directory
            (opts.existing, opts.missing)
        } else {
            // automatically create directories if the open options try creating the final file/directory
            (
                ExistingPolicy::UseExisting,
                if opts.missing.is_readonly() {
                    MissingPolicy::Error
                } else {
                    MissingPolicy::CreateDir
                },
            )
        };

        let next = dir_node_entry(&cur_dir, part, existing, missing).await?;
        cur = next;
    }

    Ok(cur)
}

pub async fn dir_node_entry(
    dir: &DirNode,
    part: &str,
    existing: ExistingPolicy,
    missing: MissingPolicy,
) -> Result<Node> {
    // try get the next node from cached nodes
    let mut cache = dir.nodes.lock().await;
    if let Some(cached) = cache.get(part) {
        match existing {
            ExistingPolicy::UseExisting => return Ok(cached.clone()),
            ExistingPolicy::Error => return Err(Error::ALREADY_EXISTS),
        }
    }

    // try get the next node from the concrete filesystem
    let driver = dir.driver.lock().await;
    // TODO: use `_part_to_end` to await on `driver.get()` only once per node find
    // because `driver.get()` has to create a boxed future
    match driver.get(part).await {
        Ok((found_node, can_cache)) => {
            if can_cache {
                cache.insert(part.into(), found_node.clone());
            }

            match existing {
                ExistingPolicy::UseExisting => return Ok(found_node.clone()),
                ExistingPolicy::Error => return Err(Error::ALREADY_EXISTS),
            }
        }
        Err(Error::NOT_FOUND) => {}
        Err(other) => return Err(other),
    }

    // the file wasnt in the cache nor in the concrete filesystem
    match missing {
        MissingPolicy::CreateDir => {
            let (dir, can_cache) = driver.create_dir(part).await?;
            if can_cache {
                cache.insert(part.into(), dir.clone());
            }
            Ok(dir)
        }
        MissingPolicy::CreateFile => {
            let (file, can_cache) = driver.create_file(part).await?;
            if can_cache {
                cache.insert(part.into(), file.clone());
            }
            Ok(file)
        }
        MissingPolicy::Error => Err(Error::NOT_FOUND),
    }
}

/* use alloc::{boxed::Box, collections::BTreeMap, sync::Arc};
use core::{
    marker::Unsize,
    ops::{CoerceUnsized, Deref},
    ptr::NonNull,
    str,
    sync::atomic::{AtomicUsize, Ordering},
};

use async_trait::async_trait;
use hyperion_futures::lock::Mutex;
use hyperion_syscall::err::{Error, Result};

//

extern crate alloc;

//

//

// pub struct GraphIter<'a> {
//     inner: PathIter<'a>,
//     cur: Option<Ref<Node>>,
// }

// impl<'a> Iterator for GraphIter<'a> {
//     type Item = Result<(Ref<Node>, &'a str)>;

//     fn next(&mut self) -> Option<Self::Item> {
//         let cur = self.cur.as_ref()?;

//         let (part, remainder) = self.inner.next()?;
//         let Some(cur_dir) = cur.as_dir() else {
//             self.cur = None;
//             return Some(Err(Error::NOT_A_DIRECTORY));
//         };

//         cur_dir;

//         todo!()
//     }
// }

//

#[derive(Debug, Clone, Copy)]
pub struct OpenOptions {
    pub create_dirs: bool,
    pub missing_behaviour: MissingBehaviour,
}

#[derive(Debug, Clone, Copy)]
pub enum MissingBehaviour {
    /// the file has to already exist
    Existing,
    /// the file has to NOT exist and it will be created
    New,
    /// the file will be created if it doesnt exist
    ExistingOrNew,
}

impl OpenOptions {
    pub const fn new() -> Self {
        Self {
            create_dirs: false,
            missing_behaviour: MissingBehaviour::Existing,
        }
    }

    pub fn create_dirs(&mut self, create_dirs: bool) -> &mut Self {
        self.create_dirs = create_dirs;
        self
    }

    /// this is the default
    pub fn existing(&mut self) -> &mut Self {
        self.missing_behaviour = MissingBehaviour::Existing;
        self
    }

    pub fn existing_or_new(&mut self) -> &mut Self {
        self.missing_behaviour = MissingBehaviour::ExistingOrNew;
        self
    }

    pub fn create_new(&mut self) -> &mut Self {
        self.missing_behaviour = MissingBehaviour::New;
        self
    }

    pub async fn open(&self, path: &str) -> Result<Ref<Node>> {
        open(path, *self).await
    }
}

impl Default for OpenOptions {
    fn default() -> Self {
        Self::new()
    }
}

//

pub async fn open(path: &str, opts: OpenOptions) -> Result<Ref<Node>> {
    let (device, device_relative_path) = find_device(path).await?;
    device.get(device_relative_path, opts).await
}

pub async fn mount() {}

pub async fn entry(path: &str, opts: OpenOptions) -> Result<()> {
    let mut cur = ROOT.clone();
    let mut device = None;
    let mut device_relative_path = "";

    let mut path = path.split('/');
    let file_name = path.next_back();

    for part in path {
        let cur_dir = cur.as_dir().ok_or(Error::NOT_A_DIRECTORY)?;

        if cur_dir.device.is_some() {
            device = cur_dir.device.clone();
        }

        let mut cur_dir_entries = cur_dir.mounts.nodes.lock().await;

        if let Some(entry) = cur_dir_entries.get(part).cloned() {
            drop(cur_dir_entries);
            cur = entry;
        } else if opts.create_dirs {
            let new_dir = Ref::from(Node::new_none());
            cur_dir_entries.insert(part.into(), new_dir.clone());
            drop(cur_dir_entries);
            cur = new_dir;
        } else {
            return Err(Error::NOT_FOUND);
        };
    }

    let parent_dir = cur.as_dir().ok_or(Error::NOT_A_DIRECTORY)?;

    if parent_dir.device.is_some() {
        device = parent_dir.device.clone();
    }

    let device = device;

    Ok(())
}

struct QueryResult<'a> {
    last_mounted: QueryItem<'a>,
    last_node: QueryItem<'a>,
    parent: Option<QueryItem<'a>>,
}

struct QueryItem<'a> {
    node: Ref<Node>,
    relative_path: &'a str,
}

/// finds the directory device mounted with the most precision,
/// and returns that device and the relative path in that device
///
/// for example `/boot/efi/EFI/BOOT/BOOTX64.EFI` would return
///  - a handle to the device that represents `/boot/efi`
///  - a path `EFI/BOOT/BOOTX64.EFI`
pub async fn find_device(path: &str) -> Result<(Ref<dyn DirDevice + Send + Sync>, &str)> {
    let mut cur = ROOT.clone();
    let mut device = None;
    let mut device_relative_path = "";

    for (part, remainder) in PathIter::new(path) {
        let cur_dir = cur.as_dir().ok_or(Error::NOT_A_DIRECTORY)?;
        if cur_dir.device.is_some() {
            device = cur_dir.device.clone();
            device_relative_path = remainder;
        }

        let Some(next) = cur_dir.mounts.nodes.lock().await.get(part).cloned() else {
            break;
        };

        cur = next;
    }

    Ok((device.ok_or(Error::NOT_FOUND)?, device_relative_path))
}

async fn query(path: &str) -> Result<QueryResult> {
    let mut res = QueryResult {
        last_mounted: QueryItem {
            node: ROOT.clone(),
            relative_path: path,
        },
        last_node: QueryItem {
            node: ROOT.clone(),
            relative_path: path,
        },
        parent: None,
    };

    for (part, remainder) in PathIter::new(path) {
        let cur_dir = res.last_node.node.as_dir().ok_or(Error::NOT_A_DIRECTORY)?;
        if cur_dir.device.is_some() {
            device = cur_dir.device.clone();
            device_relative_path = remainder;
        }

        let Some(next) = cur_dir.mounts.nodes.lock().await.get(part).cloned() else {
            break;
        };

        cur = next;
    }

    Ok(res)
}

pub async fn open_file(path: &str, opts: OpenOptions) -> Result<Ref<Node>> {
    // let s: QueryResult = todo!();

    // if s.last_node_relative_path.is_empty() {
    //     if opts.missing_behaviour == MissingBehaviour::New {
    //         return Err(Error::ALREADY_EXISTS);
    //     }

    //     s.last_node.as_file().ok_or(Error::NOT_A_FILE)?;
    //     return Ok(s.last_node);
    // }

    let mut cur = ROOT.clone();
    let mut device = None;

    let mut path_iter = PathIter::new(path);
    let file_path = path_iter.file_name().ok_or(Error::NOT_A_FILE)?;

    for (part, remainder) in path_iter {
        let cur_dir = cur.as_dir().ok_or(Error::NOT_A_DIRECTORY)?;
        if let Some(new_device) = cur_dir.device.clone() {
            device = Some((new_device, remainder));
        }

        let Some(next) = cur_dir.mounts.nodes.lock().await.get(part).cloned() else {
            break;
        };
        cur = next;
    }

    let parent_dir = cur.as_dir().ok_or(Error::NOT_A_DIRECTORY)?;
    if let Some(new_device) = parent_dir.device.clone() {
        device = Some((new_device, remainder));
    }

    todo!()
}

pub async fn open_dir(path: &str, opts: OpenOptions) -> Result<Ref<Node>> {
    let mut cur = ROOT.clone();

    for (part, _) in PathIter::new(path) {
        let cur_dir = cur.as_dir().ok_or(Error::NOT_A_DIRECTORY)?;
        let Some(next) = cur_dir.mounts.nodes.lock().await.get(part).cloned() else {
            break;
        };
        cur = next;
    }

    cur.as_dir().ok_or(Error::NOT_A_DIRECTORY)?;
    Ok(cur)
}

pub async fn mount_device(path: &str, dir_dev: Ref<dyn DirDevice + Send + Sync>) -> Result<()> {
    todo!()
}

pub async fn unmount_device(path: &str) -> Result<Ref<dyn DirDevice + Send + Sync>> {
    todo!()
}

pub async fn bind_device(path: &str, file_dev: Ref<dyn FileDevice + Send + Sync>) -> Result<()> {
    todo!()
}

pub async fn unbind_device(path: &str) -> Result<Ref<dyn FileDevice + Send + Sync>> {
    todo!()
}

pub struct FindMountedDevice<'a> {
    /// the most specific mounted device,
    /// like `/boot/efi` if the search was `/boot/efi/something.txt`
    pub device: Ref<Node>,

    /// the leftover parts of a path after the mounted device has been found
    /// like `something.txt` if the search was `/boot/efi/something.txt`
    pub relative_target_path: Option<&'a str>,
}

//

static ROOT_NODE: Node = Node::new_none();
static ROOT: Ref<Node> = Ref::from_ref(&ROOT_NODE);

//

pub enum Node {
    Dir(DirNode),
    File(FileNode),
}

impl Node {
    pub const fn new_dir(device: Ref<dyn DirDevice + Send + Sync>) -> Self {
        Self::Dir(DirNode {
            mounts: Mounts::new(),
            device: Some(device),
        })
    }

    pub const fn new_none() -> Self {
        Self::Dir(DirNode {
            mounts: Mounts::new(),
            device: None,
        })
    }

    pub const fn new_file(device: Ref<dyn FileDevice + Send + Sync>) -> Self {
        Self::File(FileNode { device })
    }

    pub const fn as_dir(&self) -> Option<&DirNode> {
        match self {
            Node::Dir(dir_node) => Some(dir_node),
            _ => None,
        }
    }

    pub const fn as_file(&self) -> Option<&FileNode> {
        match self {
            Node::File(file_node) => Some(file_node),
            _ => None,
        }
    }

    // pub fn get(
    //     &self,
    //     part: &str,
    //     remainder: &str,
    //     last_device: &mut Option<(Ref<dyn DirDevice + Send + Sync>, &str)>,
    // ) -> Result<Ref<Node>> {
    //     let cur_dir = self.as_dir().ok_or(Error::NOT_A_DIRECTORY)?;
    //     if let Some(device) = cur_dir.device.clone() {
    //         *last_device = Some((device, remainder));
    //     }
    //     Ok();
    // }
}

pub struct DirNode {
    /// node graph children
    mounts: Mounts,

    /// a device mounted here, like `/boot/efi`
    device: Option<Ref<dyn DirDevice + Send + Sync>>,
}

pub struct Mounts {
    nodes: Mutex<BTreeMap<Arc<str>, Ref<Node>>>,
}

#[derive(Clone)]
pub struct FileNode {
    /// a device mounted here, like `/dev/null`
    device: Ref<dyn FileDevice + Send + Sync>,
}

impl Mounts {
    pub const fn new() -> Self {
        Self {
            nodes: Mutex::new(BTreeMap::new()),
        }
    }
}

impl Default for Mounts {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
pub trait DirDevice {
    async fn get(&self, device_relative_path: &str, opts: OpenOptions) -> Result<Ref<Node>> {
        _ = (device_relative_path, opts);
        Err(Error::PERMISSION_DENIED)
    }
}

pub trait FileDevice {}

impl DirDevice for () {}

impl FileDevice for () {}

/// custom `Arc` with const init option from a `&'static T`
pub struct Handle<T: ?Sized> {
    ptr: NonNull<HandleInner<T>>,
}

unsafe impl<T: ?Sized + Sync + Send> Send for Handle<T> {}
unsafe impl<T: ?Sized + Sync + Send> Sync for Handle<T> {}

impl<T: ?Sized + Unsize<U>, U: ?Sized> CoerceUnsized<Handle<U>> for Handle<T> {}

impl<T: 'static> Handle<T> {
    pub fn new(v: T) -> Self {
        let inner = Box::leak(Box::new(HandleInner {
            strong: AtomicUsize::new(1),
            data: v,
        }));

        Self::from_inner(inner)
    }
}

impl<T: ?Sized> Handle<T> {
    pub const fn from_inner(v: &'static HandleInner<T>) -> Self {
        Self {
            ptr: unsafe { NonNull::new_unchecked(v as *const _ as _) },
        }
    }

    fn inner(&self) -> &HandleInner<T> {
        unsafe { self.ptr.as_ref() }
    }
}

impl<T: ?Sized> Clone for Handle<T> {
    fn clone(&self) -> Self {
        let old = self.inner().strong.fetch_add(1, Ordering::Relaxed);
        if old >= 0x7FFFFFFFFFFFFFFF {
            panic!("too many Handle references");
        }

        Self { ptr: self.ptr }
    }
}

impl<T: ?Sized> Drop for Handle<T> {
    fn drop(&mut self) {
        if self.inner().strong.fetch_sub(1, Ordering::Release) != 1 {
            return;
        }

        (#[cold]
        || {})();

        _ = self.inner().strong.load(Ordering::Acquire);

        unsafe {
            core::ptr::drop_in_place(self.ptr.as_ptr());
        }
    }
}

#[doc(hidden)]
pub struct HandleInner<T: ?Sized> {
    strong: AtomicUsize,
    data: T,
}

impl<T> HandleInner<T> {
    #[doc(hidden)]
    pub const unsafe fn _create_static(v: T) -> Self {
        Self {
            strong: AtomicUsize::new(2), // as if there is one leaked handle
            data: v,
        }
    }
}

#[macro_export]
macro_rules! static_handle {
    ($($expr:tt)*) => {{
        static __STATIC_HANDLE: $crate::HandleInner = $crate::HandleInner::_create_static($($expr)*);
        $crate::Handle::from_inner(&__STATIC_HANDLE)
    }};
}

pub enum Ref<T: ?Sized + 'static> {
    Arc(Arc<T>),
    Static(&'static T),
}

impl<T: ?Sized> Ref<T> {
    pub const fn from_ref(value: &'static T) -> Self {
        Self::Static(value)
    }
}

impl<T> From<T> for Ref<T> {
    fn from(value: T) -> Self {
        Arc::new(value).into()
    }
}

impl<T: ?Sized> From<Arc<T>> for Ref<T> {
    fn from(value: Arc<T>) -> Self {
        Self::Arc(value)
    }
}

impl<T: ?Sized> From<&'static T> for Ref<T> {
    fn from(value: &'static T) -> Self {
        Self::from_ref(value)
    }
}

impl<T: ?Sized> Clone for Ref<T> {
    fn clone(&self) -> Self {
        match self {
            Ref::Arc(v) => Ref::Arc(v.clone()),
            Ref::Static(v) => Ref::Static(*v),
        }
    }
}

impl<T: ?Sized> Deref for Ref<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        match self {
            Ref::Arc(v) => v,
            Ref::Static(v) => v,
        }
    }
}

//

pub struct TmpFs {
    nodes: Arc<TmpFsDir>,
}

struct TmpFsDir {
    nodes: Mutex<BTreeMap<Box<str>, TmpFsDirEntry>>,
}

#[derive(Clone)]
enum TmpFsDirEntry {
    Dir(Arc<TmpFsDir>),
    File(FileNode),
}

impl TmpFsDirEntry {
    fn as_dir(&self) -> Result<&Arc<TmpFsDir>> {
        match self {
            TmpFsDirEntry::Dir(arc) => Ok(arc),
            TmpFsDirEntry::File(_) => Err(Error::NOT_A_DIRECTORY),
        }
    }

    fn as_file(&self) -> Result<&FileNode> {
        match self {
            TmpFsDirEntry::Dir(_) => Err(Error::NOT_A_FILE),
            TmpFsDirEntry::File(file_node) => Ok(file_node),
        }
    }
}

#[async_trait]
impl DirDevice for TmpFs {
    async fn get(&self, device_relative_path: &str) -> Result<Ref<Node>> {
        let mut cur = self.nodes.clone();

        for part in path_iter(&device_relative_path) {
            let nodes = cur.nodes.lock().await;
            let next = nodes.get(part).ok_or(Error::NOT_FOUND)?.as_dir()?.clone();
            drop(nodes);
            cur = next;
        }

        match cur {}

        cur.nodes.lock();

        while true {
            let nodes = cur.nodes.lock().await;
        }

        cur.nodes;

        _ = device_relative_path;
        Err(Error::PERMISSION_DENIED)
    }
} */
