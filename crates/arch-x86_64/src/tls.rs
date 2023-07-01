use core::{
    cell::{Ref, RefCell, RefMut},
    sync::atomic::{AtomicUsize, Ordering},
};

use hyperion_boot::cpu_count;
use hyperion_drivers::acpi::apic::ApicId;
use x86_64::{registers::model_specific::GsBase, VirtAddr};

use crate::cpu::CpuState;

//

pub fn init(tls: &'static mut RefCell<ThreadLocalStorage>) {
    /* let mut flags = Cr4::read();
    flags.insert(Cr4Flags::FSGSBASE);
    unsafe { Cr4::write(flags) };
    GS::write_base(base) */

    GsBase::write(VirtAddr::new(tls as *mut _ as usize as u64));

    INITIALIZED.fetch_add(1, Ordering::Release);
}

pub fn get() -> Ref<'static, ThreadLocalStorage> {
    get_cell().borrow()
}

pub fn get_mut() -> RefMut<'static, ThreadLocalStorage> {
    get_cell().borrow_mut()
}

pub fn get_cell() -> &'static RefCell<ThreadLocalStorage> {
    if INITIALIZED.load(Ordering::Acquire) != cpu_count() {
        panic!("TLS was not initialized for every CPU");
    }

    unsafe { &*GsBase::read().as_ptr() }
}

//

#[derive(Debug)]
pub struct ThreadLocalStorage {
    pub lapic: Option<ApicId>,
    pub cpu: CpuState,
}

//

impl ThreadLocalStorage {
    pub fn new(cpu: CpuState) -> Self {
        Self { lapic: None, cpu }
    }

    pub fn new_wrapped(cpu: CpuState) -> RefCell<ThreadLocalStorage> {
        Self::new(cpu).into()
    }
}

//

static INITIALIZED: AtomicUsize = AtomicUsize::new(0);
