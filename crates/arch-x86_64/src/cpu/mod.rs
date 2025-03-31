use alloc::boxed::Box;
use core::{
    mem::MaybeUninit,
    sync::atomic::{AtomicUsize, Ordering},
};

use gdt::Gdt;
use idt::Idt;
use spin::{Mutex, MutexGuard};
use tss::Tss;

use crate::tls::{self, ThreadLocalStorage};

//

pub mod gdt;
pub mod idt;
pub mod ints;
pub mod tss;

//

pub fn init() -> &'static ThreadLocalStorage {
    let cpu_descriptors = CpuState::new_tls();
    tls::init(cpu_descriptors);
    cpu_descriptors
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
    pub fn new_tls() -> &'static ThreadLocalStorage {
        let cpu_id = next_cpu_id();

        if let Some(tls) = Self::new_boot_tls(cpu_id) {
            // BSP
            tls
        } else {
            // other processors later
            Self::new_alloc_tls(cpu_id)
        }
    }

    fn new_boot_tls(cpu_id: usize) -> Option<&'static ThreadLocalStorage> {
        if cpu_id != 0 {
            return None;
        }

        static TLS_CELL: StaticLeak<CpuDataAlloc> = StaticLeak::new(CpuState::new_uninit());

        let uninit = TLS_CELL.leak()?;

        Some(Self::from_uninit(uninit, cpu_id))
    }

    fn new_alloc_tls(cpu_id: usize) -> &'static ThreadLocalStorage {
        // SAFETY: assume_init is safe, because each CpuDataAlloc field is MaybeUninit
        let data = unsafe { Box::<CpuDataAlloc>::new_uninit().assume_init() };

        Self::from_uninit(Box::leak(data), cpu_id)
    }

    const fn new_uninit() -> CpuDataAlloc {
        (
            MaybeUninit::uninit(),
            MaybeUninit::uninit(),
            MaybeUninit::uninit(),
            MaybeUninit::uninit(),
        )
    }

    fn from_uninit(
        (tss, gdt, idt, tls): &'static mut CpuDataAlloc,
        cpu_id: usize,
    ) -> &'static ThreadLocalStorage {
        let tss = tss.write(Tss::new());
        let gdt = gdt.write(Gdt::new(tss));
        gdt.load();
        let idt = idt.write(Idt::new(tss));
        idt.load();

        ThreadLocalStorage::init(tls, CpuState { tss, gdt, idt }, cpu_id)
    }
}

fn next_cpu_id() -> usize {
    static NEXT_CPU_ID: AtomicUsize = AtomicUsize::new(0);
    NEXT_CPU_ID.fetch_add(1, Ordering::Relaxed)
}

//

struct StaticLeak<T> {
    inner: Mutex<T>,
}

impl<T> StaticLeak<T> {
    pub const fn new(val: T) -> Self {
        Self {
            inner: Mutex::new(val),
        }
    }

    pub fn leak(&self) -> Option<&mut T> {
        let lock = self.inner.try_lock()?;
        Some(MutexGuard::leak(lock))
    }
}

unsafe impl<T> Sync for StaticLeak<T> {}
unsafe impl<T> Send for StaticLeak<T> {}
