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
use hyperion_arch::{stack::USER_HEAP_TOP, vmm::PageMap};
use hyperion_mem::vmm::{MapTarget, PageMapImpl};
use spin::{Mutex, Once};
use x86_64::{structures::paging::PageTableFlags, VirtAddr};

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

    /// process heap beginning, the end of the user process
    pub heap_bottom: AtomicUsize,

    /// TLS object data, each thread allocates one into the userspace
    /// and the $fs segment register should be set to point to it
    // pub master_tls: Once<(VirtAddr, Layout)>,

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
            heap_bottom: AtomicUsize::new(0x1000),
            ext: Once::new(),
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

    pub fn alloc(&self, n_pages: usize, flags: PageTableFlags) -> Result<VirtAddr, AllocErr> {
        let n_bytes = n_pages * 0x1000;

        let Ok(at) = VirtAddr::try_new(self.heap_bottom.fetch_add(n_bytes, Ordering::SeqCst) as _)
        else {
            return Err(AllocErr::OutOfVirtMem);
        };

        if (at + n_bytes).as_u64() >= USER_HEAP_TOP {
            return Err(AllocErr::OutOfVirtMem);
        }

        self.alloc_at_keep_heap_bottom(n_pages, at, flags)?;

        Ok(at)
    }

    pub fn alloc_at(
        &self,
        n_pages: usize,
        at: VirtAddr,
        flags: PageTableFlags,
    ) -> Result<(), AllocErr> {
        self.heap_bottom
            .fetch_max(at.as_u64() as usize + n_pages * 0x1000, Ordering::SeqCst);
        self.alloc_at_keep_heap_bottom(n_pages, at, flags)
    }

    pub fn free(&self, n_pages: usize, ptr: VirtAddr) -> Result<(), FreeErr> {
        if !self
            .address_space
            .is_mapped(ptr..ptr + n_pages * 0x1000, PageTableFlags::USER_ACCESSIBLE)
        {
            return Err(FreeErr::InvalidAlloc);
        }

        let n_bytes = n_pages * 0x1000;

        self.address_space.unmap(ptr..ptr + n_bytes);

        Ok(())
    }

    fn alloc_at_keep_heap_bottom(
        &self,
        n_pages: usize,
        at: VirtAddr,
        flags: PageTableFlags,
    ) -> Result<(), AllocErr> {
        let n_bytes = n_pages * 0x1000;

        self.address_space
            .map(at..at + n_bytes, MapTarget::LazyAlloc, flags);

        Ok(())
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
