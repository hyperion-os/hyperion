#![no_std]
#![feature(
    str_split_remainder,
    let_chains,
    stmt_expr_attributes,
    coerce_unsized,
    unsize,
    future_join,
    map_try_insert,
    arbitrary_self_types
)]

use alloc::collections::btree_map::BTreeMap;
use core::future::join;

use hyperion_futures::mutex::Mutex;
use hyperion_scheduler::proc::Process;
use hyperion_syscall::{
    err::{Error, Result},
    fs::FileOpenFlags,
};

use self::{
    node::{DirDriver, DirNode, FileDriver, FileNode, Node, Ref},
    tmpfs::TmpFs,
};

//

extern crate alloc;

//

pub mod node;
pub mod path;
pub mod tmpfs;

//

static ROOT_DEV: TmpFs = TmpFs::new();
static ROOT_NODE: DirNode = DirNode {
    nodes: Mutex::new(BTreeMap::new()),
    driver: Mutex::new(Ref::new_static(&ROOT_DEV)),
};
static ROOT: Ref<DirNode> = Ref::new_static(&ROOT_NODE);

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

    pub fn from_flags(f: FileOpenFlags) -> Self {
        let missing = if f.contains(FileOpenFlags::IS_DIR) {
            MissingPolicy::CreateDir
        } else {
            MissingPolicy::CreateFile
        };

        if f.contains(FileOpenFlags::CREATE_NEW) {
            Self {
                existing: ExistingPolicy::Error,
                missing,
            }
        } else if f.contains(FileOpenFlags::CREATE) {
            Self {
                existing: ExistingPolicy::UseExisting,
                missing,
            }
        } else {
            Self {
                existing: ExistingPolicy::UseExisting,
                missing: MissingPolicy::Error,
            }
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

pub async fn get_file(
    proc: Option<&Process>,
    path: &str,
    opts: OpenOptions,
) -> Result<Ref<FileNode>> {
    get(proc, path, opts)
        .await?
        .to_file()
        .ok_or(Error::NOT_A_FILE)
}

pub async fn get_dir(
    proc: Option<&Process>,
    path: &str,
    opts: OpenOptions,
) -> Result<Ref<DirNode>> {
    get(proc, path, opts)
        .await?
        .to_dir()
        .ok_or(Error::NOT_A_DIRECTORY)
}

pub async fn mount(
    proc: Option<&Process>,
    path: &str,
    dev: Ref<dyn DirDriver>,
) -> Result<Ref<DirNode>> {
    let node = get_dir(
        proc,
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

pub async fn unmount(proc: Option<&Process>, path: &str) -> Result<()> {
    let (parent_dir, name) = path.rsplit_once('/').unwrap_or(("", path));
    let parent = get_dir(
        proc,
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

pub async fn bind(
    proc: Option<&Process>,
    path: &str,
    dev: Ref<dyn FileDriver>,
) -> Result<Ref<FileNode>> {
    let (parent_dir, name) = path.rsplit_once('/').unwrap_or(("", path));
    let parent = get_dir(
        proc,
        parent_dir,
        OpenOptions {
            existing: ExistingPolicy::UseExisting,
            missing: MissingPolicy::CreateDir,
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

pub async fn unbind(proc: Option<&Process>, path: &str) -> Result<()> {
    unmount(proc, path).await
}

/// travel through the node graph and try to find the file/dir at `path`
///
/// if `create_dirs` is set,
/// then it creates directories every time it cannot find a node
/// (except on the root because missing root means there is no driver)
pub async fn get(proc: Option<&Process>, path: &str, opts: OpenOptions) -> Result<Node> {
    if !path.starts_with('/') {
        return Err(Error::INVALID_PATH);
    }
    let path = &path[1..];

    let mut cur = Node::Dir(ROOT.clone());

    for part in path::PathIter::new(path) {
        let (part, _part_to_end) = part?;

        let cur_dir = cur.to_dir().ok_or(Error::NOT_A_DIRECTORY)?;

        let is_last = part == _part_to_end;
        // hyperion_log::info!("part={part} _part_to_end={_part_to_end} is_last={is_last}");
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

        let next = dir_node_entry(proc, &cur_dir, part, existing, missing).await?;
        cur = next;
    }

    Ok(cur)
}

pub async fn dir_node_entry(
    proc: Option<&Process>,
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
    match driver.get(proc, part).await {
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
            let (dir, can_cache) = driver.create_dir(proc, part).await?;
            if can_cache {
                cache.insert(part.into(), dir.clone());
            }
            Ok(dir)
        }
        MissingPolicy::CreateFile => {
            let (file, can_cache) = driver.create_file(proc, part).await?;
            if can_cache {
                cache.insert(part.into(), file.clone());
            }
            Ok(file)
        }
        MissingPolicy::Error => Err(Error::NOT_FOUND),
    }
}
