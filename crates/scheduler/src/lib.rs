#![no_std]
#![feature(slice_ptr_get)]

extern crate alloc;

//

use hyperion_arch::{
    cpu::ints::PAGE_FAULT_HANDLER,
    vmm::{PageMap, HIGHER_HALF_DIRECT_MAPPING},
};
use hyperion_mem::vmm::{NotHandled, PageFaultResult, PageMapImpl, Privilege};
use x86_64::VirtAddr;

use self::{proc::Process, task::RunnableTask};

//

// pub mod buf;
pub mod proc;
pub mod task;

//

// /// terminate the active task and enter the async scheduler
pub fn init() {
    PAGE_FAULT_HANDLER.store(page_fault_handler);
}

fn page_fault_handler(_ip: usize, addr: usize, privilege: Privilege) -> PageFaultResult {
    hyperion_log::trace!("page fault ip={_ip:x} addr={addr:x} priv={privilege:?}");

    if privilege == Privilege::Kernel && addr >= HIGHER_HALF_DIRECT_MAPPING.as_u64() as usize {
        // modify the global kernel maps
        // FIXME: lock the global pages when fixing page faults and mapping
        if PageMap::current()
            .page_fault(VirtAddr::new(addr as u64), privilege)
            .is_handled()
        {
            return PageFaultResult::Handled;
        }
    }

    let Some(proc) = Process::current() else {
        return PageFaultResult::NotHandled;
    };

    if proc
        .address_space
        .page_fault(VirtAddr::new(addr as u64), privilege)
        .is_handled()
    {
        return PageFaultResult::Handled;
    }

    if addr <= HIGHER_HALF_DIRECT_MAPPING.as_u64() as usize {
        // TODO: sig segv
        // FIXME: syscall exit to not use the page fault stack
        hyperion_log::warn!("user-space page fault");
        hyperion_syscall::exit(0);
    }

    PageFaultResult::NotHandled
}
