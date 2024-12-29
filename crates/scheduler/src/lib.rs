#![no_std]

extern crate alloc;

//

use hyperion_arch::{
    cpu::ints::PAGE_FAULT_HANDLER,
    vmm::{PageMap, HIGHER_HALF_DIRECT_MAPPING},
};
use hyperion_mem::vmm::{NotHandled, PageFaultResult, PageMapImpl, Privilege};
use x86_64::VirtAddr;

use self::task::RunnableTask;

//

pub mod proc;
pub mod task;

//

// /// terminate the active task and enter the async scheduler
pub fn init() {
    PAGE_FAULT_HANDLER.store(page_fault_handler);
}

fn page_fault_handler(_ip: usize, addr: usize, privilege: Privilege) -> PageFaultResult {
    if privilege == Privilege::Kernel && addr >= HIGHER_HALF_DIRECT_MAPPING.as_u64() as usize {
        // modify the global kernel maps
        // FIXME: lock the global pages when fixing page faults and mapping
        PageMap::current().page_fault(VirtAddr::new(addr as u64), privilege)?;
    }

    // hyperion_log::debug!("page fault ip={_ip:x} addr={addr:x}");
    let Some(active) = task::Task::take_active() else {
        return Ok(NotHandled);
    };
    let proc = active.process.clone();
    active.set_active();

    proc.address_space
        .page_fault(VirtAddr::new(addr as u64), privilege)?;

    if addr <= HIGHER_HALF_DIRECT_MAPPING.as_u64() as usize {
        // TODO: sig segv
        // FIXME: syscall exit to not use the page fault stack
        RunnableTask::next().enter();
        // unreachable
    }

    Ok(NotHandled)
}
