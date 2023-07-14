use hyperion_arch::{syscall::SyscallRegs, vmm::PageMap};
use hyperion_mem::vmm::PageMapImpl;
use x86_64::{structures::paging::PageTableFlags, VirtAddr};

//

pub fn syscall(args: &mut SyscallRegs) {
    match args.syscall_id {
        // syscall `log`
        1 => {
            let Some(end) = args.arg0.checked_add(args.arg1) else {
                args.syscall_id = 1;
                return;
            };

            let (start, end) = (VirtAddr::new(args.arg0), VirtAddr::new(end));

            if PageMap::current().is_mapped(start..end, PageTableFlags::USER_ACCESSIBLE) {
                args.syscall_id = 2;
                return;
            }

            // TODO:
            // SAFETY: this is most likely unsafe
            let str: &[u8] =
                unsafe { core::slice::from_raw_parts(start.as_ptr(), end.as_u64() as _) };

            let Ok(str) = core::str::from_utf8(str) else {
                args.syscall_id = 3;
                return;
            };

            hyperion_log::println!("{str}");
            args.syscall_id = 0;
        }

        // syscall `exit` (also syscall `commit_oxygen_not_reach_lungs`)
        2 | 420 => {
            args.syscall_id = 0;

            // TODO: impl real exit instead of just halting the cpu

            hyperion_arch::done();
        }

        _ => {
            // invalid syscall id, kill the process as a f u
            hyperion_arch::done();
        }
    }
}
