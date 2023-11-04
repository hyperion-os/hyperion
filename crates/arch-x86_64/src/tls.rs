use core::{
    mem::MaybeUninit,
    ptr::{addr_of_mut, null_mut},
    sync::atomic::{AtomicPtr, Ordering},
};

use x86_64::{
    registers::model_specific::{GsBase, KernelGsBase},
    VirtAddr,
};

//

pub fn init(tls: &'static ThreadLocalStorage) {
    tls.kernel_stack
        .store(VirtAddr::new_truncate(0).as_mut_ptr(), Ordering::SeqCst);

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
