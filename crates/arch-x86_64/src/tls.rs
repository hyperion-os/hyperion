use alloc::boxed::Box;
use core::{
    mem::MaybeUninit,
    ops::Deref,
    ptr::{addr_of_mut, null_mut},
    sync::atomic::{AtomicBool, AtomicPtr, AtomicUsize, Ordering},
};

use crossbeam::queue::SegQueue;
use hyperion_mem::{pmm, vmm::Privilege};
use hyperion_scheduler_task::{CleanupTask, Task};
use spin::Mutex;
use x86_64::{
    registers::{
        model_specific::{GsBase, KernelGsBase},
        segmentation::GS,
    },
    VirtAddr,
};

use crate::{cpu_count, cpu_id};

//

pub struct Tls<T: 'static> {
    inner: Box<[T]>,
}

impl<T: 'static> Tls<T> {
    pub fn new(mut f: impl FnMut() -> T) -> Self {
        Self {
            inner: (0..cpu_count()).map(|_| f()).collect(),
        }
    }
}

impl<T: 'static> Deref for Tls<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.inner[cpu_id()]
    }
}

//

pub fn init(tls: &'static ThreadLocalStorage) {
    /* let mut flags = Cr4::read();
    flags.insert(Cr4Flags::FSGSBASE);
    unsafe { Cr4::write(flags) };
    GS::write_base(base) */

    // TODO: use the current stack
    let stack: &'static mut [u8] = pmm::PFA.alloc(5).leak();

    tls.kernel_stack
        .store(stack.as_mut_ptr_range().end, Ordering::SeqCst);

    hyperion_log::debug!("TLS: 0x{:016x}", tls as *const _ as usize);

    // in kernel space, GS points to thread local storage
    GsBase::write(VirtAddr::new(tls as *const _ as usize as u64));
    // and before entering userland `swapgs` is used so that
    // in user space, GS points to user data
    KernelGsBase::write(VirtAddr::new_truncate(0));

    INITIALIZED.fetch_add(1, Ordering::Release);
}

pub fn get() -> &'static ThreadLocalStorage {
    // TODO: unsafe
    /* if INITIALIZED.load(Ordering::Acquire) != cpu_count() {
        panic!("TLS was not initialized for every CPU");
    } */

    let tls = GsBase::read();

    assert_ne!(tls.as_u64(), 0);

    unsafe { &*tls.as_ptr() }
}

pub fn dbg() {
    let gs = GsBase::read();
    hyperion_log::debug!("GS:{gs:016x}");
    unsafe { GS::swap() }
    let gs = GsBase::read();
    hyperion_log::debug!("GSSWAP:{gs:016x}");
    unsafe { GS::swap() }
}

/// # Safety
///
/// - has to be called from kernel code
/// - should be called only once from an interrupt
pub unsafe fn interrupt_gs_guard(privilege: Privilege) -> GsGuard {
    GsGuard::new(privilege)
}

//

#[repr(align(0x1000))]
pub struct ThreadLocalStorage {
    // temporary store for user space stack
    pub user_stack: AtomicPtr<u8>,
    // kernel stack for syscalls
    pub kernel_stack: AtomicPtr<u8>,
    pub active: Mutex<Option<Task>>,
    pub after_switch: SegQueue<CleanupTask>,
    pub can_yield: AtomicBool,
}

macro_rules! uninit_write_fields {
    ($uninit_struct:expr, $struct_name:ident {
        $($field_name:ident: $field_value:expr),* $(,)?
    }) => {{
        let uninit = $uninit_struct;
        let ptr = uninit.as_mut_ptr();
        unsafe {
            $(
                addr_of_mut!((*ptr).$field_name).write($field_value);
            )*
        }

        // a compile time remider to add missing field initializers
        #[allow(unused)]
        if let Some($struct_name {
            $($field_name),*
        }) = None
        {}

        unsafe { uninit.assume_init_ref() }
    }};
}

impl ThreadLocalStorage {
    pub fn init(uninit_tls: &mut MaybeUninit<Self>) -> &Self {
        uninit_write_fields!(
            uninit_tls,
            Self {
                user_stack: AtomicPtr::new(null_mut()),
                kernel_stack: AtomicPtr::new(null_mut()),
                active: Mutex::new(None),
                after_switch: SegQueue::new(),
                can_yield: AtomicBool::new(false),
            }
        )
    }
}

//

pub struct GsGuard {
    privilege: Privilege,
}

impl GsGuard {
    /// # Safety
    ///
    /// - has to be called from kernel code
    /// - should be called only once from an interrupt
    pub unsafe fn new(privilege: Privilege) -> Self {
        if privilege == Privilege::User {
            // hyperion_log::debug!("gsswap");
            unsafe { GS::swap() }
        }

        Self { privilege }
    }
}

impl Drop for GsGuard {
    fn drop(&mut self) {
        if self.privilege == Privilege::User {
            // hyperion_log::debug!("drop gsswap");
            unsafe { GS::swap() }
        }
    }
}

//

static INITIALIZED: AtomicUsize = AtomicUsize::new(0);
