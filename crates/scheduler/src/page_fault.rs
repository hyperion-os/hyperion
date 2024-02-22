use core::sync::atomic::Ordering;

use hyperion_log::*;
use hyperion_mem::vmm::{NotHandled, PageFaultResult, PageMapImpl, Privilege};
use x86_64::VirtAddr;

use crate::{exit, task, task::TaskInner, tls};

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

        // let page_map = PageMap::current();
        let page_map = &current.address_space.page_map;

        page_map.page_fault(v_addr, user)?;

        // try handling the page fault first if it happened during a task switch
        if user == Privilege::User {
            // `Err(Handled)` short circuits and returns
            // handle_stack_grow(&current.address_space.page_map, &current.user_stack, addr)?;
        } else {
            // handle_stack_grow(&current.address_space.page_map, &current.kernel_stack, addr)?;
            // handle_stack_grow(&current.address_space.page_map, &current.user_stack, addr)?;
        }

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
        // `Err(Handled)` short circuits and returns
        // handle_stack_grow(&current.address_space.page_map, &current.user_stack, addr)?;

        // user process tried to access memory thats not available to it
        let maps_to = current.address_space.page_map.virt_to_phys(v_addr);
        warn!("SIGSEGV (PID:{pid} TID:{tid}) #{addr:#x} @{instr:#x}",);
        warn!("{addr:#x} maps to {maps_to:#x?}");
        current.should_terminate.store(true, Ordering::SeqCst);
        exit();
    } else {
        // handle_stack_grow(&current.address_space.page_map, &current.kernel_stack, addr)?;
        // handle_stack_grow(&current.address_space.page_map, &current.user_stack, addr)?;

        let maps_to = current.address_space.page_map.virt_to_phys(v_addr);
        error!("kernel SIGSEGV (PID:{pid} TID:{tid}) #{addr:#x} @{instr:#x}");
        error!("{addr:#x} maps to {maps_to:#x?}");
    };

    Ok(NotHandled)
}

// fn handle_stack_grow<T: StackType + Debug>(
//     _page_map: &PageMap,
//     stack: &Mutex<Stack<T>>,
//     addr: usize,
// ) -> PageFaultResult {
//     // let page_map = PageMap::current(); // technically maybe perhaps possibly UB
//     stack.lock().page_fault(&page_map, addr as u64)
// }
