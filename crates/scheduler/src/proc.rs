use alloc::{
    boxed::Box,
    collections::BTreeMap,
    sync::{Arc, Weak},
};
use core::{
    any::Any,
    fmt,
    sync::atomic::{AtomicU64, AtomicUsize, Ordering},
};

use arcstr::{literal, ArcStr};
use crossbeam::epoch::Atomic;
use hyperion_arch::vmm::PageMap;
use hyperion_futures::mutex::Mutex as FutMutex;
use hyperion_mem::vmm::PageMapImpl;
use spin::{Mutex, Once};
use x86_64::VirtAddr;

use crate::task::{Task, Tid};

//

// TODO: get rid of the slow dumbass spinlock mutexes everywhere
pub static PROCESSES: Mutex<BTreeMap<Pid, Weak<Process>>> = Mutex::new(BTreeMap::new());

//

/// A process, each process can have multiple 'tasks' (pthreads)
pub struct Process {
    /// process id
    pub pid: Pid,

    /// next thread id
    pub next_tid: AtomicUsize,

    /// number of threads in this process
    pub threads: AtomicUsize,

    /// process name
    pub name: Atomic<ArcStr>,

    /// cpu time this process (all tasks) has used in nanoseconds
    pub nanos: AtomicU64,

    /// process address space
    pub address_space: PageMap,

    /// process maps
    pub maps: FutMutex<BTreeMap<VirtAddr, ()>>,

    // /// TLS object data, each thread allocates one into the userspace
    // /// and the $fs segment register should be set to point to it
    // pub master_tls: Once<(VirtAddr, Layout)>,

    // FIXME: static type instead of this Box dyn
    /// extra process info added by the kernel (like file descriptors)
    pub ext: Once<Box<dyn ProcessExt + 'static>>,
    // /// exit code if the process already exit
    // pub exit_code: crate::lock::Once<ExitCode>,
}

impl Process {
    pub fn new() -> Arc<Self> {
        let this = Arc::new(Self {
            pid: Pid::next(),
            next_tid: AtomicUsize::new(0),
            threads: AtomicUsize::new(0),
            name: Atomic::new(literal!("uninitialized-process")),
            nanos: AtomicU64::new(0),
            address_space: PageMap::new(),
            maps: FutMutex::new(BTreeMap::new()),
            ext: Once::new(),
        });

        PROCESSES.lock().insert(this.pid, Arc::downgrade(&this));

        this
    }

    pub async fn fork(&self, new_ext: Box<dyn ProcessExt>) -> Arc<Self> {
        let this = Arc::new(Self {
            pid: Pid::next(),
            next_tid: AtomicUsize::new(0),
            threads: AtomicUsize::new(0),
            name: self.name.clone(),
            nanos: self.nanos.load(Ordering::Acquire).into(),
            address_space: self.address_space.fork(),
            maps: FutMutex::new(self.maps.lock().await.clone()),
            ext: Once::initialized(new_ext),
        });

        PROCESSES.lock().insert(this.pid, Arc::downgrade(&this));

        this
    }

    pub fn current() -> Option<Arc<Self>> {
        Some(Task::current()?.process.clone())
    }

    pub fn next_tid(&self) -> Tid {
        Tid::new(self.next_tid.fetch_add(1, Ordering::Relaxed))
    }
}

impl Drop for Process {
    fn drop(&mut self) {
        // hyperion_log::debug!("dropping process '{}'", self.name.get_mut());
        PROCESSES.lock().remove(&self.pid);
    }
}

//

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Pid(usize);

impl Pid {
    pub const fn new(num: usize) -> Self {
        Self(num)
    }

    pub fn next() -> Self {
        static NEXT_PID: AtomicUsize = AtomicUsize::new(1);
        Self::new(NEXT_PID.fetch_add(1, Ordering::Relaxed))
    }

    pub const fn num(self) -> usize {
        self.0
    }
}

impl Pid {
    pub fn find(self) -> Option<Arc<Process>> {
        PROCESSES
            .lock()
            .get(&self)
            .and_then(|mem_weak_ref| mem_weak_ref.upgrade())
    }
}

impl fmt::Display for Pid {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

//

pub trait ProcessExt: Sync + Send {
    fn as_any(&self) -> &dyn Any;

    /// close everything before the actual process closes,
    /// because there might be no tasks to switch to (and that would keep this open)
    fn close(&self);
}

//

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AllocErr {
    OutOfVirtMem,
    // TODO:
    // OutOfMem,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FreeErr {
    InvalidAddr,
    InvalidAlloc,
}
