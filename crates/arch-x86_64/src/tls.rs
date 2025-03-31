#[cfg(debug_assertions)]
use core::cell::Cell;
use core::{
    cell::UnsafeCell,
    mem::{self, offset_of, MaybeUninit},
    ptr::{addr_of_mut, null_mut, NonNull},
    sync::atomic::AtomicPtr,
};

use x86_64::{
    registers::model_specific::{GsBase, KernelGsBase},
    VirtAddr,
};

use crate::cpu::CpuState;

//

pub fn init(tls: &'static ThreadLocalStorage) {
    // in kernel space, GS points to thread local storage
    // KernelGsBase::write(VirtAddr::new(tls as *const _ as usize as u64));
    // and before entering userland `swapgs` is used so that
    // in user space, GS points to user data
    GsBase::write(VirtAddr::new(tls as *const _ as usize as u64));
    // GsBase::write(VirtAddr::new_truncate(0));
}

//

#[repr(C, align(0x1000))]
pub struct ThreadLocalStorage {
    pub self_ptr: NonNull<Self>,

    /// temporary store for user space stack
    pub user_stack: AtomicPtr<u8>,
    /// kernel stack for syscalls
    pub kernel_stack: AtomicPtr<u8>,

    pub cpu_id: usize,

    // FIXME: merge most things back into one crate
    /// scheduler specific opaque data
    sched_opaque: UnsafeCell<MaybeUninit<[usize; 2]>>,
    #[cfg(debug_assertions)]
    sched_opaque_init: Cell<bool>,

    // FIXME: merge most things back into one crate
    /// local APIC specific opaque data
    lapic_opaque: UnsafeCell<MaybeUninit<[usize; 2]>>,
    #[cfg(debug_assertions)]
    lapic_opaque_init: Cell<bool>,

    /// GDT + IDT + TSS
    pub cpu: CpuState,
}

impl ThreadLocalStorage {
    pub const USER_STACK: usize = offset_of!(Self, user_stack);
    pub const KERNEL_STACK: usize = offset_of!(Self, kernel_stack);

    /// # Safety
    /// modifies "immutable" data
    pub unsafe fn init_sched_opaque<T>(&self, val: T) {
        let slot =
            unsafe { Self::get_opaque_mut::<_, MaybeUninit<T>>(&mut *self.sched_opaque.get()) };
        slot.write(val);

        #[cfg(debug_assertions)]
        self.sched_opaque_init.set(true);
    }

    /// # Safety
    /// modifies "immutable" data
    pub unsafe fn init_lapic_opaque<T>(&self, val: T) {
        let slot =
            unsafe { Self::get_opaque_mut::<_, MaybeUninit<T>>(&mut *self.lapic_opaque.get()) };
        slot.write(val);

        #[cfg(debug_assertions)]
        self.lapic_opaque_init.set(true);
    }

    /// # Safety
    ///  - this function can only ever be called with one
    /// specific type that matches the field in size and alignment
    ///
    ///  - the matching init has to have been called before
    pub unsafe fn get_sched_opaque<T>(&self) -> &T {
        #[cfg(debug_assertions)]
        assert!(self.sched_opaque_init.get());
        unsafe { Self::get_opaque(&*self.sched_opaque.get()) }
    }

    /// # Safety
    /// - this function can only ever be called with one
    /// specific type that matches the field in size and alignment
    ///
    ///  - the matching init has to have been called before
    pub unsafe fn get_lapic_opaque<T>(&self) -> &T {
        #[cfg(debug_assertions)]
        assert!(self.lapic_opaque_init.get());
        unsafe { Self::get_opaque(&*self.lapic_opaque.get()) }
    }

    unsafe fn get_opaque<T, U>(from: &MaybeUninit<T>) -> &U {
        assert_eq!(mem::size_of::<T>(), mem::size_of::<U>());
        assert_eq!(mem::align_of::<T>(), mem::align_of::<U>());

        unsafe { &*from.as_ptr().cast::<U>() }
    }

    unsafe fn get_opaque_mut<T, U>(from: &mut MaybeUninit<T>) -> &mut U {
        assert_eq!(mem::size_of::<T>(), mem::size_of::<U>());
        assert_eq!(mem::align_of::<T>(), mem::align_of::<U>());

        unsafe { &mut *from.as_mut_ptr().cast::<U>() }
    }
}

macro_rules! uninit_write_fields {
    ($uninit_struct:expr, $struct_name:ident {
        $($(#[$($cfg:tt)*])* $field_name:ident: $field_value:expr),* $(,)?
    }) => {{
        let uninit = $uninit_struct;
        let ptr = uninit.as_mut_ptr();
        unsafe {
            $(
                $(#[$($cfg)*])*
                addr_of_mut!((*ptr).$field_name).write($field_value);
            )*
        }

        // a compile time remider to add missing field initializers
        #[allow(unused)]
        if let Some($struct_name {
            $(
                $(#[$($cfg)*])*
                $field_name
            ),*
        }) = None
        {}

        unsafe { uninit.assume_init_ref() }
    }};
}

impl ThreadLocalStorage {
    pub fn init(uninit_tls: &mut MaybeUninit<Self>, state: CpuState, cpu_id: usize) -> &Self {
        let self_ptr: NonNull<Self> = NonNull::new(uninit_tls.as_mut_ptr()).unwrap();
        uninit_write_fields!(
            uninit_tls,
            Self {
                self_ptr: self_ptr,
                user_stack: AtomicPtr::new(null_mut()),
                kernel_stack: AtomicPtr::new(null_mut()),
                cpu_id: cpu_id,
                sched_opaque: UnsafeCell::new(MaybeUninit::uninit()),
                #[cfg(debug_assertions)]
                sched_opaque_init: Cell::new(false),
                lapic_opaque: UnsafeCell::new(MaybeUninit::uninit()),
                #[cfg(debug_assertions)]
                lapic_opaque_init: Cell::new(false),
                cpu: state,
            }
        )
    }
}
