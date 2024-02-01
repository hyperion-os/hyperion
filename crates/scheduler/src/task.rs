use alloc::{
    boxed::Box,
    collections::BTreeMap,
    sync::{Arc, Weak},
    vec,
    vec::Vec,
};
use core::{
    alloc::Layout,
    any::{type_name_of_val, Any},
    cell::UnsafeCell,
    fmt, mem,
    ops::Deref,
    ptr,
    sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering},
};

use arcstr::ArcStr;
use crossbeam::atomic::AtomicCell;
use hyperion_arch::{
    context::{switch as ctx_switch, Context},
    stack::{AddressSpace, KernelStack, Stack, UserStack, USER_HEAP_TOP},
    vmm::PageMap,
};
use hyperion_bitmap::Bitmap;
use hyperion_log::*;
use hyperion_mem::{
    pmm::{self, PageFrame},
    vmm::PageMapImpl,
};
use hyperion_sync::TakeOnce;
use spin::{Mutex, MutexGuard, Once, RwLock};
use x86_64::{
    align_up, registers::model_specific::FsBase, structures::paging::PageTableFlags, PhysAddr,
    VirtAddr,
};

use crate::{after, cleanup::Cleanup, done, swap_current, task, tls};

//

// static MAGIC_DEBUG_BYTE: Lazy<usize> = Lazy::new(|| hyperion_random::next_fast_rng().gen());

// TODO: get rid of the slow dumbass spinlock mutexes everywhere
pub static PROCESSES: Mutex<BTreeMap<Pid, Weak<Process>>> = Mutex::new(BTreeMap::new());
// pub static TASKS: Mutex<Vec<Weak<Process>>> = Mutex::new(Vec::new());

pub static TASKS_RUNNING: AtomicUsize = AtomicUsize::new(0);
pub static TASKS_SLEEPING: AtomicUsize = AtomicUsize::new(0);
pub static TASKS_READY: AtomicUsize = AtomicUsize::new(0);
pub static TASKS_DROPPING: AtomicUsize = AtomicUsize::new(0);

//

pub fn processes() -> Vec<Arc<Process>> {
    let processes = PROCESSES.lock();
    // processes.retain(|_, proc| proc.upgrade().is_some());

    processes.values().filter_map(Weak::upgrade).collect()
}

#[track_caller]
pub fn switch_because(next: Task, new_state: TaskState, cleanup: Cleanup) {
    // debug!("switching to {}", next.name.read().clone());
    if !next.is_valid {
        panic!("this task is not safe to switch to");
    }

    let next_ctx = next.context.get();
    if next.swap_state(TaskState::Running) == TaskState::Running {
        panic!("this task is already running");
    }

    if let Some(tls) = next.tls.get().copied() {
        FsBase::write(tls);
        // unsafe { FS::write_base(tls) };
    }

    // tell the page fault handler that the actual current task is still this one
    {
        let task = task();
        let task_inner: &TaskInner = &task;
        tls().switch_last_active.store(
            task_inner as *const TaskInner as *mut TaskInner,
            Ordering::SeqCst,
        );
        drop(task);
    }

    let prev = swap_current(next);
    let prev_ctx = prev.context.get();
    if prev.swap_state(new_state) != TaskState::Running {
        panic!("previous task wasn't running");
    }

    // push the current thread to the drop queue AFTER switching
    after().push(cleanup.task(prev));

    // SAFETY: `prev` is stored in the queue, `next` is stored in the TLS
    // the box keeps the pointer pinned in memory
    debug_assert!(tls().initialized.load(Ordering::SeqCst));
    unsafe { ctx_switch(prev_ctx, next_ctx) };

    // the ctx_switch can continue either in `thread_entry` or here:

    post_ctx_switch();
}

fn post_ctx_switch() {
    // invalidate the page fault handler's old task store
    tls()
        .switch_last_active
        .store(ptr::null_mut(), Ordering::SeqCst);

    crate::cleanup();

    // hyperion_arch::
}

extern "C" fn thread_entry() -> ! {
    post_ctx_switch();

    let task = task();
    let job = task.job.take().expect("no active jobs");
    task.init_tls();
    drop(task);

    job();

    done();
}

//

#[derive(Debug, Default)]
pub struct PageAllocs {
    // TODO: a better way to keep track of allocated pages
    bitmap: Once<Mutex<Bitmap<'static>>>,
}

impl PageAllocs {
    pub fn bitmap(&self) -> MutexGuard<Bitmap<'static>> {
        self.bitmap
            .call_once(|| {
                let bitmap_alloc = vec![0u8; pmm::PFA.bitmap_len() / 8];
                Mutex::new(Bitmap::new(Vec::leak(bitmap_alloc)))
            })
            .lock()
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
        fmt::Display::fmt(&self.0, f)
    }
}

//

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Tid(usize);

impl Tid {
    pub const fn new(num: usize) -> Self {
        Self(num)
    }

    pub fn next(proc: &Process) -> Self {
        Self::new(proc.next_tid.fetch_add(1, Ordering::Relaxed))
    }

    pub const fn num(self) -> usize {
        self.0
    }
}

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
    pub name: RwLock<ArcStr>,

    /// cpu time this process (all tasks) has used in nanoseconds
    pub nanos: AtomicU64,

    /// process address space
    pub address_space: AddressSpace,

    /// process heap beginning, the end of the user process
    pub heap_bottom: AtomicUsize,

    // TODO: the AddressSpace already knows these
    /// a store for all allocated (and mapped) physical pages
    pub allocs: PageAllocs,

    /// TLS object data, each thread allocates one into the userspace
    /// and the $fs segment register should be set to point to it
    pub master_tls: Once<(VirtAddr, Layout)>,

    /// extra process info added by the kernel (like file descriptors)
    pub ext: Once<Box<dyn ProcessExt + 'static>>,

    pub should_terminate: AtomicBool,
}

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

impl Process {
    pub fn new(pid: Pid, name: ArcStr, address_space: AddressSpace) -> Arc<Self> {
        let this = Arc::new(Self {
            pid,
            next_tid: AtomicUsize::new(0),
            threads: AtomicUsize::new(1),
            name: RwLock::new(name),
            nanos: AtomicU64::new(0),
            address_space,
            heap_bottom: AtomicUsize::new(0x1000),
            allocs: PageAllocs::default(),
            master_tls: Once::new(),
            ext: Once::new(),
            should_terminate: AtomicBool::new(false),
        });

        PROCESSES.lock().insert(this.pid, Arc::downgrade(&this));

        this
    }

    pub fn alloc(
        &self,
        n_pages: usize,
        flags: PageTableFlags,
    ) -> Result<(VirtAddr, PhysAddr), AllocErr> {
        let n_bytes = n_pages * 0x1000;

        let Ok(at) = VirtAddr::try_new(self.heap_bottom.fetch_add(n_bytes, Ordering::SeqCst) as _)
        else {
            return Err(AllocErr::OutOfVirtMem);
        };

        if (at + n_bytes).as_u64() >= USER_HEAP_TOP {
            return Err(AllocErr::OutOfVirtMem);
        }

        let phys = self.alloc_at_keep_heap_bottom(n_pages, at, flags)?;

        Ok((at, phys))
    }

    pub fn alloc_at(
        &self,
        n_pages: usize,
        at: VirtAddr,
        flags: PageTableFlags,
    ) -> Result<PhysAddr, AllocErr> {
        self.heap_bottom
            .fetch_max(at.as_u64() as usize + n_pages * 0x1000, Ordering::SeqCst);
        self.alloc_at_keep_heap_bottom(n_pages, at, flags)
    }

    pub fn free(&self, n_pages: usize, ptr: VirtAddr) -> Result<(), FreeErr> {
        let mut bitmap = self.allocs.bitmap();

        let Some(palloc) = self.address_space.page_map.virt_to_phys(ptr) else {
            return Err(FreeErr::InvalidAddr);
        };

        let page_bottom = palloc.as_u64() as usize / 0x1000;
        for page in page_bottom..page_bottom + n_pages {
            if !bitmap.get(page).unwrap() {
                return Err(FreeErr::InvalidAlloc);
            }

            bitmap.set(page, false).unwrap();
        }

        let frames = unsafe { PageFrame::new(palloc, n_pages) };
        pmm::PFA.free(frames);
        self.address_space
            .page_map
            .unmap(ptr..ptr + n_pages * 0x1000);

        Ok(())
    }

    fn alloc_at_keep_heap_bottom(
        &self,
        n_pages: usize,
        at: VirtAddr,
        flags: PageTableFlags,
    ) -> Result<PhysAddr, AllocErr> {
        let n_bytes = n_pages * 0x1000;

        let alloc_bottom = at;
        let alloc_top = at + n_bytes;

        // TODO: split into 1 page chunks
        let physical_pages = pmm::PFA.alloc(n_pages);

        self.address_space.page_map.map(
            alloc_bottom..alloc_top,
            physical_pages.physical_addr(),
            flags,
        );

        let page_bottom = physical_pages.physical_addr().as_u64() as usize / 0x1000;

        let mut bitmap = self.allocs.bitmap();
        for page in page_bottom..page_bottom + n_pages {
            bitmap.set(page, true).unwrap();
        }

        Ok(physical_pages.physical_addr())
    }
}

impl Drop for Process {
    fn drop(&mut self) {
        // hyperion_log::debug!("dropping process '{}'", self.name.get_mut());

        PROCESSES.lock().remove(&self.pid);

        let Some(bitmap) = self.allocs.bitmap.get_mut() else {
            return;
        };
        let bitmap = bitmap.get_mut();

        for page in bitmap.iter_true() {
            let phys_page = unsafe { PageFrame::new(PhysAddr::new(page as u64 * 0x1000), 1) };
            pmm::PFA.free(phys_page);
        }

        // the page map is dropped, so unmapping pages isn't needed
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

pub struct TaskInner {
    /// thread id
    ///
    /// thread id's are per process, each process has at least TID 0
    pub tid: Tid,

    /// a shared process ref, multiple tasks can point to the same process
    pub process: Arc<Process>,

    /// task state, 'is the task waiting or what?'
    pub state: AtomicCell<TaskState>,

    /// lazy initialized user-space stack
    pub user_stack: Mutex<Stack<UserStack>>,

    /// lazy initialized kernel-space stack,
    /// also used when entering kernel-space from a `syscall` but not from a interrupt
    /// each CPU has its privilege stack in TSS, page faults also have their own stacks per CPU
    pub kernel_stack: Mutex<Stack<KernelStack>>,

    /// thread_entry runs this function once, and stops the process after returning
    pub job: TakeOnce<Box<dyn FnOnce() + Send + 'static>>,

    /// a copy of the master TLS for specifically this task
    pub tls: Once<VirtAddr>,

    // context is used 'unsafely' only in the switch
    // TaskInner is pinned in heap using a Box to make sure a pointer to this (`context`)
    // is valid after switching task before switching context
    context: UnsafeCell<Context>,

    // context is valid to switch to only if this is true
    is_valid: bool,
}

impl TaskInner {
    pub fn init_tls(&self) {
        _ = self.tls.try_call_once(|| {
            let Some((addr, layout)) = self.master_tls.get().copied() else {
                // master tls doesn't exist, so don't copy it
                return Err(());
            };

            // TODO: deallocate it when the thread quits

            // afaik, it has to be at least one page?
            // and at least 2 pages if TLS has any data
            // , because FSBase has to point to TCB and
            // FSBase has to be page aligned and TLS
            // has to be right below TCB
            // +--------------+--------------+
            // |   PHYS PAGE  |  PHYS PAGE   |
            // +--------+-----+-----+--------+
            // | unused | TLS | TCB | unused |
            // +--------+-----+-----+--------+

            // the alloc is align_up(TLS) + TCB
            let tls_alloc = align_up(layout.size() as u64, 0x10u64);
            let n_pages = tls_alloc.div_ceil(0x1000) as usize + 1;
            let (tls_copy, _) = self
                .alloc(
                    n_pages,
                    PageTableFlags::USER_ACCESSIBLE
                        | PageTableFlags::WRITABLE
                        | PageTableFlags::PRESENT,
                )
                .unwrap();

            let tcb_ptr = tls_copy + (n_pages - 1) * 0x1000;
            let tls_ptr = tcb_ptr - tls_alloc;

            debug!("TLS copy base={tls_copy:#018x}");

            // copy the master TLS
            unsafe {
                ptr::copy_nonoverlapping::<u8>(addr.as_ptr(), tls_ptr.as_mut_ptr(), layout.size());
            }

            // init TCB
            // %fs register value, it points to a pointer to itself, TLS items are right before it
            let fs = tcb_ptr;
            // %fs:0x0 should point to fsbase
            unsafe { *fs.as_mut_ptr() = fs.as_u64() };

            // TODO: a more robust is_active fn
            let active = task();
            debug!("TLS tid:{} FS={fs:#018x}", active.tid.num());
            if self.tid == active.tid && self.pid == active.pid {
                FsBase::write(fs);
                // unsafe { FS::write_base(tls_copy) };
            }

            Ok(tls_copy)
        });
    }
}

impl Deref for TaskInner {
    type Target = Process;

    fn deref(&self) -> &Self::Target {
        &self.process
    }
}

unsafe impl Sync for TaskInner {}

impl Drop for TaskInner {
    #[track_caller]
    fn drop(&mut self) {
        assert_eq!(
            self.state.load(),
            TaskState::Dropping,
            "{}",
            self.name.read().clone(),
        );

        // hyperion_log::debug!("dropping task {:?} of '{}'", self.tid, self.name.read());

        self.threads.fetch_sub(1, Ordering::Relaxed);

        let ks = mem::take(&mut self.kernel_stack).into_inner();
        self.address_space.kernel_stacks.free(ks);
        let ks = mem::take(&mut self.user_stack).into_inner();
        self.address_space.user_stacks.free(ks);

        TASKS_DROPPING.fetch_sub(1, Ordering::Relaxed);
    }
}

//

#[derive(Clone)]
pub struct Task(Arc<TaskInner>);

impl Task {
    pub fn new(f: impl FnOnce() + Send + 'static) -> Task {
        let name = type_name_of_val(&f);
        Self::new_any(Box::new(f) as _, name.into())
    }

    pub fn new_any(f: Box<dyn FnOnce() + Send + 'static>, name: ArcStr) -> Task {
        trace!("initializing task {name}");

        let process = Process::new(Pid::next(), name, AddressSpace::new(PageMap::new()));

        let kernel_stack = process.address_space.take_kernel_stack_prealloc(1);
        let stack_top = kernel_stack.top;
        let user_stack = process.address_space.take_user_stack_lazy();

        let context = UnsafeCell::new(Context::new(
            &process.address_space.page_map,
            stack_top,
            thread_entry,
        ));

        trace!(
            "task rsp:{stack_top:#x} cr3:{:#x}",
            process.address_space.page_map.cr3().start_address()
        );

        TASKS_READY.fetch_add(1, Ordering::Relaxed);
        Self(Arc::new(TaskInner {
            tid: Tid::next(&process),
            process,
            state: AtomicCell::new(TaskState::Ready),
            user_stack: Mutex::new(user_stack),
            kernel_stack: Mutex::new(kernel_stack),
            job: TakeOnce::new(f),
            tls: Once::new(),
            context,
            is_valid: true,
        }))
    }

    pub fn thread(process: Arc<Process>, f: impl FnOnce() + Send + 'static) -> Task {
        Self::thread_any(process, Box::new(f))
    }

    pub fn thread_any(process: Arc<Process>, f: Box<dyn FnOnce() + Send + 'static>) -> Task {
        trace!(
            "initializing secondary thread for process {}",
            process.name.read().clone()
        );

        process.threads.fetch_add(1, Ordering::Relaxed);

        let kernel_stack = process.address_space.take_kernel_stack_prealloc(1);
        let stack_top = kernel_stack.top;
        let user_stack = process.address_space.take_user_stack_lazy();

        let context = UnsafeCell::new(Context::new(
            &process.address_space.page_map,
            stack_top,
            thread_entry,
        ));

        TASKS_READY.fetch_add(1, Ordering::Relaxed);
        Self(Arc::new(TaskInner {
            tid: Tid::next(&process),
            process,
            state: AtomicCell::new(TaskState::Ready),
            user_stack: Mutex::new(user_stack),
            kernel_stack: Mutex::new(kernel_stack),
            job: TakeOnce::new(f),
            tls: Once::new(),
            context,
            is_valid: true,
        }))
    }

    pub fn bootloader() -> Task {
        // TODO: dropping this task should also free the bootloader stacks
        // they are currently dropped by a task in kernel/src/main.rs

        trace!("initializing bootloader task");

        let process = Process::new(
            Pid(0),
            "bootloader".into(),
            AddressSpace::new(PageMap::current()),
        );
        process.should_terminate.store(true, Ordering::Release);

        let mut kernel_stack = process
            .address_space
            .kernel_stacks
            .take_lazy(&process.address_space.page_map);
        let mut user_stack = process
            .address_space
            .user_stacks
            .take_lazy(&process.address_space.page_map);
        kernel_stack.limit_4k_pages = 0;
        user_stack.limit_4k_pages = 0;

        // SAFETY: this task is unsafe to switch to,
        // switching is allowed only if `self.is_valid()` returns true
        let context = UnsafeCell::new(unsafe { Context::invalid(&process.address_space.page_map) });

        TASKS_RUNNING.fetch_add(1, Ordering::Relaxed);
        Self(Arc::new(TaskInner {
            tid: Tid::next(&process),
            process,
            state: AtomicCell::new(TaskState::Running),
            user_stack: Mutex::new(user_stack),
            kernel_stack: Mutex::new(kernel_stack),
            job: TakeOnce::none(),
            tls: Once::new(),
            context,
            is_valid: false,
        }))
    }

    pub fn swap_state(&self, new: TaskState) -> TaskState {
        match new {
            TaskState::Running => &TASKS_RUNNING,
            TaskState::Sleeping => &TASKS_SLEEPING,
            TaskState::Ready => &TASKS_READY,
            TaskState::Dropping => &TASKS_DROPPING,
        }
        .fetch_add(1, Ordering::Relaxed);

        let old = self.state.swap(new);

        match old {
            TaskState::Running => &TASKS_RUNNING,
            TaskState::Sleeping => &TASKS_SLEEPING,
            TaskState::Ready => &TASKS_READY,
            TaskState::Dropping => &TASKS_DROPPING,
        }
        .fetch_sub(1, Ordering::Relaxed);

        old
    }

    // pub fn ptr_eq(&self, other: &Task) -> bool {
    //     Arc::ptr_eq(&self.0, &other.0)
    // }
}

impl Deref for Task {
    type Target = TaskInner;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<F> From<F> for Task
where
    F: FnOnce() + Send + 'static,
{
    fn from(value: F) -> Self {
        Self::new(value)
    }
}

//

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskState {
    Running,
    Sleeping,
    Ready,
    Dropping,
}

const _: () = assert!(AtomicCell::<TaskState>::is_lock_free());

impl TaskState {
    pub const fn as_str(self) -> &'static str {
        match self {
            TaskState::Running => "running",
            TaskState::Sleeping => "sleeping",
            TaskState::Ready => "ready",
            TaskState::Dropping => "dropping",
        }
    }

    /// Returns `true` if the task state is [`Running`].
    ///
    /// [`Running`]: TaskState::Running
    #[must_use]
    pub fn is_running(&self) -> bool {
        matches!(self, Self::Running)
    }

    /// Returns `true` if the task state is [`Sleeping`].
    ///
    /// [`Sleeping`]: TaskState::Sleeping
    #[must_use]
    pub fn is_sleeping(&self) -> bool {
        matches!(self, Self::Sleeping)
    }

    /// Returns `true` if the task state is [`Ready`].
    ///
    /// [`Ready`]: TaskState::Ready
    #[must_use]
    pub fn is_ready(&self) -> bool {
        matches!(self, Self::Ready)
    }

    /// Returns `true` if the task state is [`Dropping`].
    ///
    /// [`Dropping`]: TaskState::Dropping
    #[must_use]
    pub fn is_dropping(&self) -> bool {
        matches!(self, Self::Dropping)
    }
}
