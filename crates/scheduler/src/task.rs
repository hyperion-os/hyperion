use alloc::sync::Arc;
use core::{cell::Cell, fmt, sync::atomic::Ordering};

use hyperion_arch::syscall::SyscallRegs;
use hyperion_cpu_id::Tls;
use hyperion_futures::mpmc::Channel;
use hyperion_mem::vmm::PageMapImpl;
use spin::Lazy;

use crate::proc::Process;

//

pub static TASKS: Channel<RunnableTask> = Channel::new();
pub static CPU: Lazy<Tls<Cpu>> = Lazy::new(Tls::default);

//

pub struct Cpu {
    pub active: Cell<Option<Task>>,
}

impl Cpu {
    pub const fn new() -> Self {
        Self {
            active: Cell::new(None),
        }
    }
}

impl Default for Cpu {
    fn default() -> Self {
        Self::new()
    }
}

//

pub struct RunnableTask {
    pub trap: SyscallRegs,
    pub task: Task,
}

impl RunnableTask {
    pub fn new(ip: u64, sp: u64) -> Self {
        Self::new_in(ip, sp, Process::new())
    }

    pub fn new_in(ip: u64, sp: u64, process: Arc<Process>) -> Self {
        // SAFETY: the task is in user space,
        // the safety is ensured by the hardware
        // (user space apps cannot touch the kernel memory)
        //
        // So none of the Rust's rules can be broken (except
        // for the user space process, it is their problem)

        let trap = SyscallRegs::new(ip, sp);
        let tid = process.next_tid();
        let task = Task { tid, process };
        Self { trap, task }
    }

    pub fn active(trap: SyscallRegs) -> Self {
        let task = Task::take_active().unwrap();
        Self { trap, task }
    }

    pub fn enter(self) -> ! {
        let mut s = self.set_active();
        hyperion_log::debug!("enter ip={:x} sp={:x}", s.user_instr_ptr, s.user_stack_ptr);
        s.enter()
    }

    pub fn set_active(self) -> SyscallRegs {
        let RunnableTask { trap, task } = self;
        task.process.address_space.debug();
        task.process.address_space.activate();
        CPU.active.replace(Some(task));
        trap
    }

    /// wait for the next ready task, and run other tasks meanwhile
    pub fn next() -> RunnableTask {
        hyperion_futures::block_on(TASKS.recv())
    }

    /// mark the task as ready to run
    pub fn ready(self) {
        TASKS.send(self);
    }
}

//

pub struct Task {
    /// thread id
    ///
    /// thread id's are local to a process
    pub tid: Tid,

    /// a shared process ref, multiple tasks can point to the same process
    pub process: Arc<Process>,
    // a copy of the master TLS for specifically this task
    // pub tls: Once<VirtAddr>,
}

impl Task {
    pub fn take_active() -> Option<Self> {
        CPU.active.take()
    }

    pub fn set_active(self) {
        CPU.active.set(Some(self));
    }

    /* pub fn init_tls(&self) {
        let fs = self
            .tls
            .try_call_once(|| {
                let Some((addr, layout)) = self.master_tls.get().copied() else {
                    // master tls doesn't exist, so don't copy it
                    return Err(());
                };
                // debug!("init_tls for {}", self.name.read());

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
                let tls_copy = self
                    .alloc(
                        n_pages,
                        PageTableFlags::USER_ACCESSIBLE | PageTableFlags::WRITABLE,
                    )
                    .unwrap();

                let tcb_ptr = tls_copy + (n_pages - 1) * 0x1000;
                let tls_ptr = tcb_ptr - tls_alloc;

                // copy the master TLS
                unsafe {
                    ptr::copy_nonoverlapping::<u8>(
                        addr.as_ptr(),
                        tls_ptr.as_mut_ptr(),
                        layout.size(),
                    );
                }

                // init TCB
                // %fs register value, it points to a pointer to itself, TLS items are right before it
                let fs = tcb_ptr;
                // %fs:0x0 should point to fsbase
                unsafe { *fs.as_mut_ptr() = fs.as_u64() };

                Ok(fs)
            })
            .ok()
            .copied()
            .unwrap_or(VirtAddr::new(0));

        // TODO: a more robust is_active fn
        let active = task();
        if self.tid == active.tid && self.pid == active.pid {
            // debug!("tls fs={fs:#018x}");
            FsBase::write(fs);
            // unsafe { FS::write_base(tls_copy) };
        }
    } */
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

impl fmt::Display for Tid {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}
