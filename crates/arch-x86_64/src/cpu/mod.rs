use alloc::boxed::Box;
use core::{cell::RefCell, mem::MaybeUninit};

use hyperion_boot_interface::Cpu;
use hyperion_log::trace;
use spin::{Mutex, MutexGuard, Once};

use self::{gdt::Gdt, idt::Idt, tss::Tss};
use crate::tls::{self, ThreadLocalStorage};

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
    Tss,
    MaybeUninit<Gdt>,
    MaybeUninit<Idt>,
    MaybeUninit<RefCell<ThreadLocalStorage>>,
);

impl CpuState {
    fn new_boot_tls() -> &'static mut RefCell<ThreadLocalStorage> {
        static BOOT_DATA: Once<Mutex<CpuDataAlloc>> = Once::new();

        let lock = BOOT_DATA
            .call_once(|| Mutex::new(Self::new_uninit()))
            .try_lock()
            .expect("cpu structures already initialized");

        Self::from_uninit(MutexGuard::leak(lock))
    }

    fn new_tls() -> &'static mut RefCell<ThreadLocalStorage> {
        Self::from_uninit(Box::leak(Box::new(Self::new_uninit())))
    }

    fn new_uninit() -> CpuDataAlloc {
        (
            Tss::new(),
            MaybeUninit::<Gdt>::uninit(),
            MaybeUninit::<Idt>::uninit(),
            MaybeUninit::<RefCell<ThreadLocalStorage>>::uninit(),
        )
    }

    fn from_uninit(
        (tss, gdt, idt, tls): &'static mut CpuDataAlloc,
    ) -> &'static mut RefCell<ThreadLocalStorage> {
        let gdt = gdt.write(Gdt::new(tss));
        gdt.load();
        let idt = idt.write(Idt::new(tss));
        idt.load();

        let cpu = Self { tss, gdt, idt };

        tls.write(ThreadLocalStorage::new_wrapped(cpu))
    }
}
