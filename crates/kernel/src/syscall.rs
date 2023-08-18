use hyperion_arch::{syscall::SyscallRegs, vmm::PageMap};
use hyperion_mem::vmm::PageMapImpl;
use x86_64::{structures::paging::PageTableFlags, VirtAddr};

//

pub fn syscall(args: &mut SyscallRegs) {
    // hyperion_log::debug!("got syscall with args: {args:#?}");

    let id = args.syscall_id;
    let (result, name) = match id {
        1 => (log(args), "log"),

        2 => (exit(args), "exit"),

        420 => (exit(args), "commit_oxygen_not_reach_lungs"),

        _ => {
            // invalid syscall id, kill the process as a f u
            args.syscall_id = 2;
            args.arg0 = i64::MIN as _;
            (exit(args), "invalid")
        }
    };

    if result != 0 {
        hyperion_log::debug!("syscall `{name}` (id {id}) returned {result}",);
    }
    args.syscall_id = result;
}

/// print a string to logs
///
/// # arguments
/// - syscall_id : 1
/// - arg0 : _utf8 string address_
/// - arg1 : _utf8 string length_
/// - arg2 : _ignored_
/// - arg3 : _ignored_
/// - arg4 : _ignored_
///
/// # return codes (in syscall_id after returning)
///  - 0 : ok
///  - 1 : invalid address range (arg0 .. arg1)
///  - 2 : address range not mapped for the user (arg0 .. arg1)
///  - 3 : invalid utf8
pub fn log(args: &mut SyscallRegs) -> u64 {
    let Some(end) = args.arg0.checked_add(args.arg1) else {
        return 1;
    };

    let (Ok(start), Ok(end)) = (
        VirtAddr::try_new(args.arg0),
        VirtAddr::try_new(end),
    ) else {
        return 1;
    };

    if !PageMap::current().is_mapped(start..end, PageTableFlags::USER_ACCESSIBLE) {
        return 2;
    }

    // TODO:
    // SAFETY: this is most likely unsafe
    let str: &[u8] = unsafe { core::slice::from_raw_parts(start.as_ptr(), args.arg1 as _) };

    let Ok(str) = core::str::from_utf8(str) else {
        return 3;
    };

    hyperion_log::print!("{str}");

    0
}

/// exit and kill the current process
///
/// # arguments
/// - syscall_id : 2
/// - arg0 : _exit code_
/// - arg1 : _ignored_
/// - arg2 : _ignored_
/// - arg3 : _ignored_
/// - arg4 : _ignored_
///
/// # return codes (in syscall_id after returning)
/// _won't return_
pub fn exit(args: &mut SyscallRegs) -> u64 {
    // TODO: impl actual exit

    args.user_instr_ptr = 0;

    0
    // hyperion_arch::done()
}
