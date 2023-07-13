use alloc::boxed::Box;
use core::{
    mem::MaybeUninit,
    ptr::{addr_of_mut, null_mut},
    sync::atomic::AtomicPtr,
};

use crossbeam::atomic::AtomicCell;
use hyperion_boot_interface::Cpu;
use hyperion_log::trace;
use hyperion_mem::vmm::PageMapImpl;
use spin::{Mutex, MutexGuard};

use self::{gdt::Gdt, idt::Idt, tss::Tss};
use crate::{
    tls::{self, ThreadLocalStorage},
    vmm::PageMap,
};

//

pub mod gdt;
pub mod idt;
pub mod ints;
pub mod tss;

//

pub fn init(cpu: &Cpu) {
    trace!("Loading CpuState for {cpu}");
    let tls = if cpu.is_boot() {
        // boot cpu doesn't need to allocate
        CpuState::new_boot_tls()
    } else {
        // other cpus have to allocate theirs
        CpuState::new_tls()
    };

    tls::init(tls);
}

//

#[derive(Debug, Clone, Copy)]
pub struct CpuState {
    pub tss: &'static Tss,
    pub gdt: &'static Gdt,
    pub idt: &'static Idt,
}

type CpuDataAlloc = (
    MaybeUninit<Tss>,
    MaybeUninit<Gdt>,
    MaybeUninit<Idt>,
    MaybeUninit<ThreadLocalStorage>,
);

impl CpuState {
    fn new_boot_tls() -> &'static ThreadLocalStorage {
        static BOOT_DATA: Mutex<CpuDataAlloc> = Mutex::new(CpuState::new_uninit());

        let lock = BOOT_DATA
            .try_lock()
            .expect("boot cpu structures already initialized");

        Self::from_uninit(MutexGuard::leak(lock))
    }

    fn new_tls() -> &'static ThreadLocalStorage {
        // SAFETY: assume_init is safe, because each CpuDataAlloc field is MaybeUninit
        let data = unsafe { Box::<CpuDataAlloc>::new_uninit().assume_init() };

        Self::from_uninit(Box::leak(data))
    }

    const fn new_uninit() -> CpuDataAlloc {
        (
            MaybeUninit::uninit(),
            MaybeUninit::uninit(),
            MaybeUninit::uninit(),
            MaybeUninit::uninit(),
        )
    }

    fn from_uninit((tss, gdt, idt, tls): &'static mut CpuDataAlloc) -> &'static ThreadLocalStorage {
        let tss = tss.write(Tss::new());
        let gdt = gdt.write(Gdt::new(tss));
        gdt.load();
        let idt = idt.write(Idt::new(tss));
        idt.load();

        // // SAFETY: assume_init_mut is safe, because each ThreadLocalStorageUninit field is MaybeUninit
        // let tls: &mut ThreadLocalStorageUninit = unsafe { tls.assume_init_mut() };

        let tlsp = tls.as_mut_ptr();
        unsafe {
            addr_of_mut!((*tlsp).user_stack).write(AtomicPtr::new(null_mut()));
            addr_of_mut!((*tlsp).kernel_stack).write(AtomicPtr::new(null_mut()));
            addr_of_mut!((*tlsp).current_page_map).write(AtomicCell::new(PageMap::current()));
        }

        // just a compile time remider to add missing field initializers
        #[allow(unused)]
        if let Some(ThreadLocalStorage {
            user_stack,
            kernel_stack,
            current_page_map,
        }) = None
        {}

        // SAFETY: assume_init_ref is safe, because each field in ThreadLocalStorage is initializd
        unsafe { tls.assume_init_ref() }
    }
}
