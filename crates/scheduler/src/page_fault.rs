use core::{fmt::Debug, sync::atomic::Ordering};

use hyperion_arch::{
    stack::{Stack, StackType},
    vmm::PageMap,
};
use hyperion_log::*;
use hyperion_mem::vmm::{NotHandled, PageFaultResult, PageMapImpl, Privilege};
use spin::Mutex;
use x86_64::VirtAddr;

use crate::{stop, task::TaskInner, task_try, TLS};

//

pub fn page_fault_handler(addr: usize, user: Privilege) -> PageFaultResult {
    trace!("scheduler page fault (from {user:?})");

    let actual_current = TLS.switch_last_active.load(Ordering::SeqCst);
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

    let current = task_try().expect("TODO: active task is locked");

    if user == Privilege::User {
        // `Err(Handled)` short circuits and returns
        handle_stack_grow(&current.user_stack, addr)?;

        // user process tried to access memory thats not available to it
        hyperion_log::warn!("killing user-space process");
        stop();
    } else {
        handle_stack_grow(&current.kernel_stack, addr)?;
        handle_stack_grow(&current.user_stack, addr)?;

        hyperion_log::error!("{:?}", current.kernel_stack.lock());
        hyperion_log::error!("page fault from kernel-space");
    };

    let page = PageMap::current();
    let v = VirtAddr::new(addr as _);
    let p = page.virt_to_phys(v);
    error!("{v:018x?} -> {p:018x?}");

    Ok(NotHandled)
}

fn handle_stack_grow<T: StackType + Debug>(
    stack: &Mutex<Stack<T>>,
    addr: usize,
) -> PageFaultResult {
    let page_map = PageMap::current(); // technically maybe perhaps possibly UB
    stack.lock().page_fault(&page_map, addr as u64)
}
