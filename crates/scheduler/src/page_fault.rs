use core::{fmt::Debug, sync::atomic::Ordering};

use hyperion_arch::{
    stack::{Stack, StackType},
    vmm::PageMap,
};
use hyperion_cpu_id::cpu_id;
use hyperion_log::*;
use hyperion_mem::vmm::{NotHandled, PageFaultResult, PageMapImpl, Privilege};
use spin::Mutex;
use x86_64::VirtAddr;

use crate::{exit, task, task::TaskInner, tls};

//

pub fn page_fault_handler(instr: usize, addr: usize, user: Privilege) -> PageFaultResult {
    // debug!(
    //     "scheduler page fault at {instr:#x} accessing {addr:#x} (from {user:?}) (cpu: {}) (pid: {})",
    //     cpu_id(),
    //     process().pid
    // );
    let v_addr = VirtAddr::new(addr as _);

    let actual_current = tls().switch_last_active.load(Ordering::SeqCst);
    if !actual_current.is_null() {
        let current: &TaskInner = unsafe { &*actual_current };

        // try handling the page fault first if it happened during a task switch
        if user == Privilege::User {
            // `Err(Handled)` short circuits and returns
            handle_stack_grow(&current.user_stack, addr)?;
        } else {
            handle_stack_grow(&current.kernel_stack, addr)?;
            handle_stack_grow(&current.user_stack, addr)?;
        }

        // otherwise fall back to handling this task's page fault
    }

    let current = task();

    if user == Privilege::User {
        // `Err(Handled)` short circuits and returns
        handle_stack_grow(&current.user_stack, addr)?;

        // current.address_space.page_map.page_fault(v_addr, user)?;

        // user process tried to access memory thats not available to it
        let maps_to = current.address_space.page_map.virt_to_phys(v_addr);
        hyperion_log::warn!(
            "killing user-space process, pid:{} tid:{} tried to use {addr:#x} at {instr:#x}",
            current.pid.num(),
            current.tid.num(),
        );
        hyperion_log::warn!("{addr:#x} maps to {maps_to:#x?}");
        current.should_terminate.store(true, Ordering::SeqCst);
        exit();
    } else {
        handle_stack_grow(&current.kernel_stack, addr)?;
        handle_stack_grow(&current.user_stack, addr)?;

        hyperion_log::error!("{:?}", current.kernel_stack.lock());
        hyperion_log::error!("page fault from kernel-space");
    };

    let maps_to = current.address_space.page_map.virt_to_phys(v_addr);
    hyperion_log::error!("{v_addr:#x} maps to {maps_to:#x?}");
    error!("couldn't handle a page fault {}", cpu_id());

    Ok(NotHandled)
}

fn handle_stack_grow<T: StackType + Debug>(
    stack: &Mutex<Stack<T>>,
    addr: usize,
) -> PageFaultResult {
    let page_map = PageMap::current(); // technically maybe perhaps possibly UB
    stack.lock().page_fault(&page_map, addr as u64)
}
