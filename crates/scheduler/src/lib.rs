#![no_std]
#![feature(let_chains, inline_const, const_nonnull_new)]

//

use core::{
    fmt::Debug,
    marker::PhantomData,
    ptr::{self, NonNull},
    sync::atomic::{AtomicPtr, Ordering},
};

use crossbeam::atomic::AtomicCell;
use crossbeam_queue::SegQueue;
use hyperion_arch::{
    cpu::ints::PAGE_FAULT_HANDLER,
    syscall::{self, SyscallRegs},
    tls::ThreadLocalStorage,
};
use hyperion_cpu_id::Tls;
use spin::{Lazy, Mutex};
use x86_64::{
    registers::{
        control::{Cr3, Cr3Flags},
        model_specific::KernelGsBase,
    },
    structures::paging::PhysFrame,
    PhysAddr,
};

//

extern crate alloc;

// pub mod cleanup;
// pub mod ipc;
// pub mod mpmc;
// pub mod condvar;
// pub mod futex;
// pub mod lock;
// pub mod proc;
// pub mod sleep;
// pub mod task;

mod page_fault;

//

// TODO: lazy allocated sparse static array using page table magic
pub static PROCESS_TABLE: [Process2; 512] = [const { Process2::stopped() }; 512];
pub static NEXT: SegQueue<&'static Process2> = SegQueue::new();
pub static PROCESSOR: Lazy<Tls<Processor>> = Lazy::new(|| Tls::new(|| Processor::new()));

//

pub fn init_bootstrap(addr_space: PhysFrame, rip: u64, rsp: u64) {
    let proc = &PROCESS_TABLE[0];

    let mut ctx = proc.context.lock();
    ctx.user_instr_ptr = rip;
    ctx.user_stack_ptr = rsp;
    drop(ctx);

    proc.addr_space.store(addr_space);

    PAGE_FAULT_HANDLER.store(page_fault::page_fault_handler);

    NEXT.push(proc);
}

pub fn process() -> &'static Process2 {
    PROCESSOR
        .active
        .load(Ordering::Acquire)
        .expect("no process is running on the CPU")
}

pub fn exit(code: ExitCode) -> ! {
    todo!("no exit in kernel code");
}

/// wait for the next ready process and start running it
pub fn done() -> ! {
    _ = &*PROCESSOR; // force init the proc table to prevent interrupts from initializing it

    loop {
        if let Some(task) = NEXT.pop() {
            // FIXME: this syscall stack retrieval code will be completely rewritten
            let tls: &'static ThreadLocalStorage = unsafe { &*KernelGsBase::read().as_ptr() };
            let rsp: u64;
            unsafe {
                core::arch::asm!("mov {rsp}, rsp", rsp = lateout(reg) rsp);
            };
            tls.kernel_stack.store(
                (rsp.div_ceil(0x1000) * 0x1000) as *mut u8,
                Ordering::Release,
            );
            task.enter();
        }
    }
}

pub struct Processor {
    /// the process that is currently running on this CPU
    pub active: AtomicRef<'static, Process2>,
    // /// the process that was running on this CPU before the current one
    // pub previous: Option<&'static Process2>,
}

impl Processor {
    pub const fn new() -> Self {
        Self {
            active: AtomicRef::new(None),
            // previous: None,
        }
    }
}

pub struct AtomicRef<'a, T> {
    ptr: AtomicPtr<T>,
    _p: PhantomData<&'a T>,
}

impl<'a, T> AtomicRef<'a, T> {
    pub const fn new(val: Option<&'a T>) -> Self {
        Self {
            ptr: AtomicPtr::new(Self::to_ptr(val)),
            _p: PhantomData,
        }
    }

    #[inline]
    pub fn load(&self, order: Ordering) -> Option<&'a T> {
        unsafe { Self::to_ref(self.ptr.load(order)) }
    }

    #[inline]
    pub fn store(&self, val: Option<&'a T>, order: Ordering) {
        self.ptr.store(Self::to_ptr(val), order);
    }

    #[inline]
    pub fn swap(&self, val: Option<&'a T>, order: Ordering) -> Option<&'a T> {
        unsafe { Self::to_ref(self.ptr.swap(Self::to_ptr(val), order)) }
    }

    const unsafe fn to_ref(ptr: *mut T) -> Option<&'a T> {
        if let Some(v) = NonNull::new(ptr) {
            Some(unsafe { v.as_ref() })
        } else {
            None
        }
    }

    const fn to_ptr(val: Option<&'a T>) -> *mut T {
        if let Some(val) = val {
            val as *const T as *mut T
        } else {
            ptr::null_mut()
        }
    }
}

// #[derive(Debug)]
pub struct Process2 {
    /// what is the process doing rn?
    pub status: AtomicCell<Status>,

    /// page table root
    pub addr_space: AtomicCell<PhysFrame>,

    /// how to continue running this process
    pub context: spin::Mutex<SyscallRegs>,
}

impl Process2 {
    pub const fn stopped() -> Self {
        // SAFETY: the process is stopped
        let context = Mutex::new(SyscallRegs::new());
        let addr_space = unsafe { PhysFrame::from_start_address_unchecked(PhysAddr::zero()) };

        Self {
            status: AtomicCell::new(Status::Stopped),
            addr_space: AtomicCell::new(addr_space),
            context,
        }
    }

    pub fn enter(&self) -> ! {
        if self.status.swap(Status::Running) == Status::Running {
            panic!("cannot run a process that is already running");
        }

        let addr_space = self.addr_space.load();
        if addr_space.start_address().as_u64() == 0 {
            panic!("PM did not set the process address space");
        }

        // switch the context address space IF it has to be switched
        if Cr3::read().0 != addr_space {
            hyperion_log::trace!("switching page maps");
            unsafe { Cr3::write(addr_space, Cr3Flags::empty()) };
        } else {
            hyperion_log::trace!("page map switch avoided (same)");
        }

        // self.addr_space.activate();

        // enter userland
        let mut context_now = *self.context.lock();
        syscall::userland_return(&mut context_now);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(C)]
pub enum Status {
    /// all process slots are stopped by default
    #[default]
    Stopped,

    /// process is waiting for a physical CPU to start running it
    Ready,

    /// process is currently running on a physical CPU
    Running,

    /// process is waiting for I/O
    Sleeping,
}

//

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ExitCode(pub i64);

impl ExitCode {
    pub const CANNOT_EXECUTE: Self = Self(126);
    pub const COMMAND_NOT_FOUND: Self = Self(127);
    pub const FATAL_SIGSEGV: Self = Self(139);
    pub const INVALID_SYSCALL: Self = Self(140);
}
