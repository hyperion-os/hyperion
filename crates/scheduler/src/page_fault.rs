use hyperion_arch::vmm::PageMap;
use hyperion_log::*;
use hyperion_mem::vmm::{PageFaultResult, PageMapImpl, Privilege};
use x86_64::VirtAddr;

use crate::{exit, ExitCode};

//

pub fn page_fault_handler(instr: usize, addr: usize, user: Privilege) -> PageFaultResult {
    // debug!(
    //     "#PF @{instr:#x} ->{addr:#x} (from {user:?}) (cpu: {})",
    //     hyperion_cpu_id::cpu_id()
    // );
    let v_addr = VirtAddr::new(addr as _);

    let pid = "??";
    let tid = pid;

    let vm = PageMap::current();
    vm.page_fault(v_addr, user)?;

    if user == Privilege::User {
        // user process tried to access memory thats not available to it
        let maps_to = vm.virt_to_phys(v_addr);
        warn!("SEGMENTATION FAULT (PID:{pid} TID:{tid}) #{addr:#x} @{instr:#x}",);
        warn!("{addr:#x} maps to {maps_to:#x?}");
    } else {
        let maps_to = vm.virt_to_phys(v_addr);
        error!("kernel SEGMENTATION FAULT (PID:{pid} TID:{tid}) #{addr:#x} @{instr:#x}");
        error!("{addr:#x} maps to {maps_to:#x?}");
        error!("trying to continue with a broken kernel");
    };

    exit(ExitCode::FATAL_SIGSEGV)
}
