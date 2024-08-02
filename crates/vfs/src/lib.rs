#![no_std]
#![feature(str_split_remainder, let_chains, map_try_insert)]

//

use alloc::{boxed::Box, collections::BTreeMap, sync::Arc};
use core::{future::Future, ops::Deref, pin::Pin};

use async_trait::async_trait;
use hyperion_futures::lock::Mutex;
use hyperion_syscall::{
    err::{Error, Result},
    fs::FileOpenFlags,
};

//

extern crate alloc;

//

// pub async fn link(path: &str, node: Ref<Node>) {}

// pub async fn set(path: &str, node: Ref<Node>) {
//     let FindMountedDevice {
//         device,
//         relative_target_path,
//     } = find_mounted_device(path).await?;
// }

/// get an atomic ref (handle) to a final node in a path
/// like whatever `something.txt` maps to in a `/boot/efi` device
pub async fn get(path: &str) -> Result<Ref<Node>> {
    let FindMountedDevice {
        device,
        relative_target_path,
    } = find_mounted_device(path).await?;

    let Some(relative_target_path) = relative_target_path else {
        return Ok(device);
    };

    match &*device {
        Node::Directory(dir) => {
            let dev = &*dir.device.as_ref().unwrap();
            dev.get(relative_target_path).await
        }
        Node::File(file) => Err(Error::NOT_A_DIRECTORY),
    }
}

pub async fn mount_device(mut path: &str, dev: Ref<Node>) -> Result<()> {
    if !path.starts_with('/') {
        hyperion_log::error!("path must be absolute starting with `/`");
        return Err(Error::FILESYSTEM_ERROR);
    }

    path = path.trim_matches('/');

    if path.is_empty() {
        // this is `/` and just mounts the root
        let mut root = ROOT.lock().await;

        if let Node::Directory(dir) = &**root
            && dir.device.is_some()
        {
            return Err(Error::ALREADY_EXISTS);
        }

        // root can be a file

        *root = dev;

        return Ok(());
    }

    let mut parts = path.split('/');
    // split always returns at least one element
    let mut last_part = parts.next_back().unwrap_or(path);
    let mut current_node = ROOT.lock().await.clone();

    // travel through everything except the last one

    for part in parts {
        let Node::Directory(dir) = &*current_node else {
            return Err(Error::NOT_A_DIRECTORY);
        };

        let mut nodes = dir.mounts.nodes.lock().await;
        if let Some(next_node) = nodes.get(part).cloned() {
            drop(nodes);
            current_node = next_node;
        } else {
            let next_node: Ref<Node> = Node::new_none().into();
            nodes.insert(part.into(), next_node.clone());

            drop(nodes);
            current_node = next_node;
        }
    }

    // insert the last one

    let Node::Directory(last_dir) = &*current_node else {
        return Err(Error::NOT_A_DIRECTORY);
    };

    let mut nodes = last_dir.mounts.nodes.lock().await;
    if nodes.try_insert(last_part.into(), dev).is_err() {
        return Err(Error::ALREADY_EXISTS);
    }

    Ok(())
}

pub struct FindMountedDevice<'a> {
    /// the most specific mounted device,
    /// like `/boot/efi` if the search was `/boot/efi/something.txt`
    pub device: Ref<Node>,

    /// the leftover parts of a path after the mounted device has been found
    /// like `something.txt` if the search was `/boot/efi/something.txt`
    pub relative_target_path: Option<&'a str>,
}

/// find the most specific mounted device and the relative path within that device
/// kinda like `df -h /boot/efi`
pub async fn find_mounted_device(path: &str) -> Result<FindMountedDevice> {
    let mut parts = path.split('/');

    let root = parts.next();
    if root != Some("") {
        hyperion_log::error!("path must be absolute starting with `/`");
        return Err(Error::FILESYSTEM_ERROR);
    }

    let mut leftover = parts.remainder();
    let mut current_node = ROOT.lock().await.clone();

    if let Node::Directory(dir) = &*current_node
        && dir.device.is_none()
    {
        hyperion_log::error!("`/` not mounted");
        return Err(Error::NOT_FOUND);
    }

    let mut last_real_mount = current_node.clone();
    let mut last_real_leftover = leftover;

    while let Some(part) = {
        leftover = parts.remainder();
        parts.next()
    } {
        // parts.remainder();
        if part.is_empty() {
            // things like ////////////////////// or //test//test become just / or /test/test
            continue;
        }

        match &*current_node {
            Node::Directory(dir) => {
                if dir.device.is_some() {
                    last_real_mount = current_node.clone();
                    last_real_leftover = leftover;
                }

                let nodes = dir.mounts.nodes.lock().await;

                let Some(next_mount_point) = nodes.get("part").cloned() else {
                    break;
                };

                drop(nodes);
                current_node = next_mount_point;
            }
            Node::File(_) => break,
        }
    }

    Ok(FindMountedDevice {
        device: current_node,
        relative_target_path: leftover,
    })
}

//

static ROOT_NODE: Node = Node::new_none();

static ROOT: Mutex<Ref<Node>> = Mutex::new(Ref::from_ref(&ROOT_NODE));

//

pub enum Node {
    Directory(DirectoryNode),
    File(FileNode),
}

impl Node {
    pub const fn new_dir(dev: Ref<dyn DirectoryDevice + Send + Sync>) -> Self {
        Self::Directory(DirectoryNode {
            mounts: Mounts::new(),
            device: Some(dev),
        })
    }

    pub const fn new_none() -> Self {
        Self::Directory(DirectoryNode {
            mounts: Mounts::new(),
            device: None,
        })
    }

    pub const fn new_file(dev: Ref<dyn FileDevice + Send + Sync>) -> Self {
        Self::File(FileNode { device: dev })
    }
}

#[derive(Clone)]
pub enum Device {
    Directory(Ref<dyn DirectoryDevice + Send + Sync>),
    File(Ref<dyn FileDevice + Send + Sync>),
}

pub struct DirectoryNode {
    mounts: Mounts,
    device: Option<Ref<dyn DirectoryDevice + Send + Sync>>,
}

pub struct FileNode {
    device: Ref<dyn FileDevice + Send + Sync>,
}

pub struct Mounts {
    nodes: Mutex<BTreeMap<Arc<str>, Ref<Node>>>,
}

impl Mounts {
    pub const fn new() -> Self {
        Self {
            nodes: Mutex::new(BTreeMap::new()),
        }
    }
}

#[async_trait]
pub trait DirectoryDevice {
    async fn get(&self, device_relative_path: &str) -> Result<Ref<Node>> {
        Err(Error::PERMISSION_DENIED)
    }
}

pub trait FileDevice {}

impl DirectoryDevice for () {}

impl FileDevice for () {}

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
