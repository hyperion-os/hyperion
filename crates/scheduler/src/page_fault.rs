use core::sync::atomic::Ordering;

use hyperion_log::*;
use hyperion_mem::vmm::{PageFaultResult, PageMapImpl, Privilege};
use x86_64::VirtAddr;

use crate::{exit, task, task::TaskInner, tls, ExitCode};

//

pub fn page_fault_handler(instr: usize, addr: usize, user: Privilege) -> PageFaultResult {
    // debug!(
    //     "#PF @{instr:#x} ->{addr:#x} (from {user:?}) (cpu: {})",
    //     hyperion_cpu_id::cpu_id()
    // );
    let v_addr = VirtAddr::new(addr as _);

    let actual_current = tls().switch_last_active.load(Ordering::SeqCst);
    if !actual_current.is_null() {
        let current: &TaskInner = unsafe { &*actual_current };

        current.address_space.page_map.page_fault(v_addr, user)?;

        // otherwise fall back to handling this task's page fault
    }

    let current = task();
    let pid = current.pid;
    let tid = current.tid;

    current
        .process
        .address_space
        .page_map
        .page_fault(v_addr, user)?;

    if user == Privilege::User {
        // user process tried to access memory thats not available to it
        let maps_to = current.address_space.page_map.virt_to_phys(v_addr);
        warn!("SEGMENTATION FAULT (PID:{pid} TID:{tid}) #{addr:#x} @{instr:#x}",);
        warn!("{addr:#x} maps to {maps_to:#x?}");
    } else {
        let maps_to = current.address_space.page_map.virt_to_phys(v_addr);
        error!("kernel SEGMENTATION FAULT (PID:{pid} TID:{tid}) #{addr:#x} @{instr:#x}");
        error!("{addr:#x} maps to {maps_to:#x?}");
        error!("trying to continue with a broken kernel");
    };

    exit(ExitCode::FATAL_SIGSEGV)
}
