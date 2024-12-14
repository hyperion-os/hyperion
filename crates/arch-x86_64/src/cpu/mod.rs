use alloc::boxed::Box;
use core::{
    cell::{RefCell, RefMut},
    mem::MaybeUninit,
};

use gdt::Gdt;
use idt::Idt;
use tss::Tss;

use crate::{
    cpu_id,
    tls::{self, ThreadLocalStorage},
};

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
    fn new_tls() -> &'static ThreadLocalStorage {
        if let Some(tls) = Self::new_boot_tls() {
            // BSP
            tls
        } else {
            // other processors later
            Self::new_alloc_tls()
        }
    }

    fn new_boot_tls() -> Option<&'static ThreadLocalStorage> {
        static BOOT_DATA: BspCell<CpuDataAlloc> = BspCell::new(CpuState::new_uninit());
        let lock = BOOT_DATA.get()?;
        Some(Self::from_uninit(RefMut::leak(lock)))
    }

    fn new_alloc_tls() -> &'static ThreadLocalStorage {
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

        ThreadLocalStorage::init(tls, CpuState { tss, gdt, idt })
    }
}

//

struct BspCell<T> {
    inner: RefCell<T>,
}

impl<T> BspCell<T> {
    const fn new(v: T) -> Self {
        BspCell {
            inner: RefCell::new(v),
        }
    }

    fn get(&self) -> Option<RefMut<T>> {
        if cpu_id() == 0 {
            Some(self.inner.borrow_mut())
        } else {
            None
        }
    }
}

unsafe impl<T> Sync for BspCell<T> {}
