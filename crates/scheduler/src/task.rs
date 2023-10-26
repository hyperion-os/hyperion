use alloc::{borrow::Cow, boxed::Box, sync::Arc, vec, vec::Vec};
use core::{
    any::type_name_of_val,
    cell::UnsafeCell,
    fmt,
    mem::ManuallyDrop,
    sync::atomic::{AtomicU64, AtomicUsize, Ordering},
};

use crossbeam::atomic::AtomicCell;
use hyperion_arch::{
    context::Context,
    stack::{AddressSpace, KernelStack, Stack, UserStack},
    vmm::PageMap,
};
use hyperion_bitmap::Bitmap;
use hyperion_log::*;
use hyperion_mem::{pmm, vmm::PageMapImpl};
use spin::{Mutex, MutexGuard, RwLock};

use crate::{thread_entry, TASKS, TASK_MEM};

//

// static MAGIC_DEBUG_BYTE: Lazy<usize> = Lazy::new(|| hyperion_random::next_fast_rng().gen());

//

pub struct TaskThread {
    pub user_stack: Mutex<Stack<UserStack>>,
    pub kernel_stack: Mutex<Stack<KernelStack>>,
}

//

pub struct TaskMemory {
    pub address_space: AddressSpace,

    pub heap_bottom: AtomicUsize,

    pub simple_ipc: Mutex<Vec<Cow<'static, [u8]>>>,
    pub simple_ipc_waiting: Mutex<Option<Task>>,

    // TODO: a better way to keep track of allocated pages
    pub allocs: Mutex<Bitmap<'static>>,
    // dbg_magic_byte: usize,
}

impl TaskMemory {
    pub fn new_arc(address_space: AddressSpace) -> Arc<Self> {
        Arc::new(Self {
            address_space,

            heap_bottom: AtomicUsize::new(0),

            simple_ipc: Mutex::new(vec![]),
            simple_ipc_waiting: Mutex::new(None),

            allocs: Mutex::new(Bitmap::new(&mut [])),
            // kernel_stack: Mutex::new(kernel_stack),
            // dbg_magic_byte: *MAGIC_DEBUG_BYTE,
        })
    }

    pub fn init_allocs(&self) {
        let bitmap_alloc: Vec<u8> = (0..pmm::PFA.bitmap_len() / 8).map(|_| 0u8).collect();
        let bitmap_alloc = Vec::leak(bitmap_alloc); // TODO: free
        *self.allocs.lock() = Bitmap::new(bitmap_alloc);
    }
}

impl Drop for TaskMemory {
    fn drop(&mut self) {
        // TODO: drop the bitmap
    }
}

//

#[derive(Debug)]
pub struct TaskInfo {
    // proc id
    pub pid: Pid,

    // proc name
    pub name: RwLock<Cow<'static, str>>,

    // cpu time used
    pub nanos: AtomicU64,

    // proc state
    pub state: AtomicCell<TaskState>,
}

const _: () = assert!(AtomicCell::<TaskState>::is_lock_free());

//

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Pid(usize);

impl Pid {
    pub const fn new(num: usize) -> Self {
        Self(num)
    }

    pub fn next() -> Self {
        static NEXT_PID: AtomicUsize = AtomicUsize::new(1);
        Pid(NEXT_PID.fetch_add(1, Ordering::Relaxed))
    }

    pub const fn num(self) -> usize {
        self.0
    }
}

impl fmt::Display for Pid {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Display::fmt(&self.0, f)
    }
}

//

pub type Task = Box<TaskInner>;

pub struct TaskInner {
    /// memory is per process
    pub memory: ManuallyDrop<Arc<TaskMemory>>,
    /// per thread
    pub thread: Box<TaskThread>,

    // context is used 'unsafely' only in the switch
    context: Box<UnsafeCell<Context>>,
    job: Option<Box<dyn FnOnce() + Send + 'static>>,

    info: Arc<TaskInfo>,

    valid: bool,
}

impl TaskInner {
    pub fn new(f: impl FnOnce() + Send + 'static) -> Self {
        let name = type_name_of_val(&f);
        let f = Box::new(f) as _;

        Self::new_any(f, Cow::Borrowed(name))
    }

    pub fn new_any(f: Box<dyn FnOnce() + Send + 'static>, name: Cow<'static, str>) -> Self {
        trace!("initializing task {name}");

        let job = Some(f);
        let name = RwLock::new(name);

        let info = Arc::new(TaskInfo {
            pid: Pid::next(),
            name,
            nanos: AtomicU64::new(0),
            state: AtomicCell::new(TaskState::Ready),
        });
        TASKS.lock().push(Arc::downgrade(&info));

        let address_space = AddressSpace::new(PageMap::new());

        let kernel_stack = address_space.take_kernel_stack_prealloc(1);
        let stack_top = kernel_stack.top;
        let main_thread_user_stack = address_space.take_user_stack_lazy();
        let thread = Box::new(TaskThread {
            user_stack: Mutex::new(main_thread_user_stack),
            kernel_stack: Mutex::new(kernel_stack),
        });

        let memory = ManuallyDrop::new(TaskMemory::new_arc(address_space));
        TASK_MEM.lock().insert(info.pid, Arc::downgrade(&memory));
        memory.init_allocs();

        let context = Box::new(UnsafeCell::new(Context::new(
            &memory.address_space.page_map,
            stack_top,
            thread_entry,
        )));

        /* let alloc = PFA.alloc(1);
        memory.address_space.page_map.map(
            CURRENT_ADDRESS_SPACE..CURRENT_ADDRESS_SPACE + 0xFFFu64,
            alloc.physical_addr(),
            PageTableFlags::PRESENT | PageTableFlags::WRITABLE,
        );
        let current_address_space: *mut Arc<TaskMemory> =
            to_higher_half(alloc.physical_addr()).as_mut_ptr();
        unsafe { current_address_space.write(memory.clone()) }; */

        Self {
            memory,
            thread,

            context,
            job,
            info,

            valid: true,
        }
    }

    pub fn thread(this: MutexGuard<'static, Task>, f: impl FnOnce() + Send + 'static) -> Self {
        Self::thread_any(this, Box::new(f))
    }

    pub fn thread_any(
        this: MutexGuard<'static, Task>,
        f: Box<dyn FnOnce() + Send + 'static>,
    ) -> Self {
        let info = this.info.clone();
        let memory = this.memory.clone();
        drop(this);

        let job = Some(f);

        let info = info.clone();

        debug!("pthread kernel stack");
        let kernel_stack = memory.address_space.take_kernel_stack_prealloc(1);
        let stack_top = kernel_stack.top;
        hyperion_log::debug!("stack top: {stack_top:018x?}");
        debug!("pthread user stack");
        let user_stack = memory.address_space.take_user_stack_lazy();
        let thread = Box::new(TaskThread {
            user_stack: Mutex::new(user_stack),
            kernel_stack: Mutex::new(kernel_stack),
        });

        debug!("pthread ctx");
        let context = Box::new(UnsafeCell::new(Context::new(
            &memory.address_space.page_map,
            stack_top,
            thread_entry,
        )));

        Self {
            memory,
            thread,

            context,
            job,
            info,

            valid: true,
        }
    }

    pub fn bootloader() -> Self {
        // TODO: dropping this task should also free the bootloader stacks
        // they are currently wasting 64KiB per processor

        let info = Arc::new(TaskInfo {
            pid: Pid(0),
            name: RwLock::new("bootloader".into()),
            nanos: AtomicU64::new(0),
            state: AtomicCell::new(TaskState::Ready),
        });

        let address_space = AddressSpace::new(PageMap::current());
        let mut kernel_stack = address_space
            .kernel_stacks
            .take_lazy(&address_space.page_map);
        let mut user_stack = address_space.user_stacks.take_lazy(&address_space.page_map);
        kernel_stack.limit_4k_pages = 0;
        user_stack.limit_4k_pages = 0;
        let thread = Box::new(TaskThread {
            user_stack: Mutex::new(user_stack),
            kernel_stack: Mutex::new(kernel_stack),
        });

        // SAFETY: this task is unsafe to switch to,
        // switching is allowed only if `self.is_valid()` returns true
        let ctx = unsafe { Context::invalid(&address_space.page_map) };
        let context = Box::new(UnsafeCell::new(ctx));

        let memory = ManuallyDrop::new(TaskMemory::new_arc(address_space));

        Self {
            memory,
            thread,

            context,
            job: None,
            info,

            valid: false,
        }
    }

    pub fn info(&self) -> &TaskInfo {
        &self.info
    }

    pub fn take_job(&mut self) -> Option<Box<dyn FnOnce() + Send + 'static>> {
        self.job.take()
    }

    pub fn debug(&mut self) {
        hyperion_log::debug!(
            "TASK DEBUG: context: {:0x?}, job: {:?}, pid: {:?}",
            &self.context as *const _ as usize,
            // unsafe { &*self.context.get() },
            self.job.as_ref().map(|_| ()),
            self.info.pid
        )
    }

    pub fn ctx(&self) -> *mut Context {
        self.context.get()
    }

    pub fn set_state(&self, state: TaskState) {
        self.info.state.store(state);
    }

    pub fn is_valid(&self) -> bool {
        self.valid
    }
}

impl<F> From<F> for Task
where
    F: FnOnce() + Send + 'static,
{
    fn from(value: F) -> Self {
        Task::new(TaskInner::new(value))
    }
}

impl Drop for TaskInner {
    fn drop(&mut self) {
        // TODO: drop pages

        // SAFETY: self.memory is not used anymore
        let memory = unsafe { ManuallyDrop::take(&mut self.memory) };

        if Arc::into_inner(memory).is_some() {
            TASK_MEM.lock().remove(&self.info.pid);
        }
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
