use alloc::boxed::Box;
use core::{
    mem::MaybeUninit,
    ops::Deref,
    ptr::{addr_of_mut, null_mut},
    sync::atomic::{AtomicPtr, Ordering},
};

use hyperion_mem::pmm;
use x86_64::{
    registers::model_specific::{GsBase, KernelGsBase},
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

    pub fn inner(this: &Self) -> &[T] {
        &this.inner
    }
}

impl<T: 'static> Deref for Tls<T> {
    type Target = T;

    #[inline(always)]
    fn deref(&self) -> &Self::Target {
        &self.inner[cpu_id()]
        // unsafe { self.inner.get_unchecked(cpu_id()) }
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
    KernelGsBase::write(VirtAddr::new(tls as *const _ as usize as u64));
    // and before entering userland `swapgs` is used so that
    // in user space, GS points to user data
    GsBase::write(VirtAddr::new_truncate(0));
}

//

#[repr(align(0x1000))]
pub struct ThreadLocalStorage {
    // temporary store for user space stack
    pub user_stack: AtomicPtr<u8>,
    // kernel stack for syscalls
    pub kernel_stack: AtomicPtr<u8>,
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
            }
        )
    }
}
