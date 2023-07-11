use core::{
    cell::{Ref, RefCell, RefMut},
    mem::transmute,
    sync::atomic::{AtomicPtr, AtomicUsize, Ordering},
};

use hyperion_boot::cpu_count;
use hyperion_mem::pmm;
use x86_64::{
    registers::model_specific::{GsBase, KernelGsBase},
    VirtAddr,
};

//

pub fn init(tls: &'static ThreadLocalStorage) {
    /* let mut flags = Cr4::read();
    flags.insert(Cr4Flags::FSGSBASE);
    unsafe { Cr4::write(flags) };
    GS::write_base(base) */

    // TODO: use the current stack
    let mut stack = pmm::PageFrameAllocator::get().alloc(5);
    let stack: &mut [u8] = stack.as_mut_slice();
    // SAFETY: the pages are never freed
    let stack: &'static mut [u8] = unsafe { transmute(stack) };

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
    if INITIALIZED.load(Ordering::Acquire) != cpu_count() {
        panic!("TLS was not initialized for every CPU");
    }

    unsafe { &*GsBase::read().as_ptr() }
}

//

#[derive(Debug)]
#[repr(align(0x1000))]
pub struct ThreadLocalStorage {
    // temporary store for user space stack
    pub user_stack: AtomicPtr<u8>,
    // kernel stack for syscalls
    pub kernel_stack: AtomicPtr<u8>,
}

//

static INITIALIZED: AtomicUsize = AtomicUsize::new(0);
