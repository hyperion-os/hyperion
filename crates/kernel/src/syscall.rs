use hyperion_arch::syscall::SyscallRegs;
use hyperion_kernel_impl::read_untrusted_str;
use hyperion_log::*;
use hyperion_scheduler::ExitCode;
use hyperion_syscall::{
    err::{Error, Result},
    id,
};

//

pub fn syscall(args: &mut SyscallRegs) {
    // static SYSCALL_RESULTS: hyperion_futures::mpmc::Channel<()> =
    //     hyperion_futures::mpmc::Channel::new();

    // process syscall args

    // dispatch / run the syscall

    // hyperion_futures::spawn(async { SYSCALL_RESULTS.send(()) });

    // block on syscall futures

    // hyperion_futures::block_on(async {
    //     let task = SYSCALL_RESULTS.recv().await;
    //     debug!("recv: {task:?}");
    // });

    // return to the same or another task

    match args.syscall_id as usize {
        id::LOG => call_id(log, args),
        id::EXIT => call_id(syscall_todo, args),
        id::DONE => call_id(syscall_todo, args),
        id::YIELD_NOW => call_id(syscall_todo, args),
        id::TIMESTAMP => call_id(syscall_todo, args),
        id::NANOSLEEP => call_id(syscall_todo, args),
        id::NANOSLEEP_UNTIL => call_id(syscall_todo, args),
        id::SPAWN => call_id(syscall_todo, args),
        id::PALLOC => call_id(syscall_todo, args),
        id::PFREE => call_id(syscall_todo, args),
        id::SEND => call_id(syscall_todo, args),
        id::RECV => call_id(syscall_todo, args),
        id::RENAME => call_id(syscall_todo, args),

        id::OPEN => call_id(syscall_todo, args),
        id::CLOSE => call_id(syscall_todo, args),
        id::READ => call_id(syscall_todo, args),
        id::WRITE => call_id(syscall_todo, args),

        id::SOCKET => call_id(syscall_todo, args),
        id::BIND => call_id(syscall_todo, args),
        id::LISTEN => call_id(syscall_todo, args),
        id::ACCEPT => call_id(syscall_todo, args),
        id::CONNECT => call_id(syscall_todo, args),

        id::GET_PID => call_id(syscall_todo, args),
        id::GET_TID => call_id(syscall_todo, args),

        id::DUP => call_id(syscall_todo, args),
        id::PIPE => call_id(syscall_todo, args),
        id::FUTEX_WAIT => call_id(syscall_todo, args),
        id::FUTEX_WAKE => call_id(syscall_todo, args),

        id::MAP_FILE => call_id(syscall_todo, args),
        id::UNMAP_FILE => call_id(syscall_todo, args),
        id::METADATA => call_id(syscall_todo, args),
        id::SEEK => call_id(syscall_todo, args),

        id::SYSTEM => call_id(syscall_todo, args),
        id::FORK => call_id(syscall_todo, args),
        id::WAITPID => call_id(syscall_todo, args),

        id::SYS_MAP_INITFS => call_id(syscall_todo, args),
        id::SYS_PROVIDE_VM => call_id(syscall_todo, args),
        id::SYS_PROVIDE_PM => call_id(syscall_todo, args),
        id::SYS_PROVIDE_VFS => call_id(syscall_todo, args),
        id::FORK_AND_EXEC => call_id(syscall_todo, args),

        id::SEND_MSG => call_id(syscall_todo, args),
        id::RECV_MSG => call_id(syscall_todo, args),
        id::SEND_RECV_MSG => call_id(syscall_todo, args),

        id::SET_GRANTS => call_id(syscall_todo, args),
        id::GRANT_READ => call_id(syscall_todo, args),
        id::GRANT_WRITE => call_id(syscall_todo, args),

        other => {
            debug!("invalid syscall ({other})");
            hyperion_scheduler::exit(ExitCode::INVALID_SYSCALL);
        }
    };
}

fn call_id(f: impl FnOnce(&mut SyscallRegs) -> Result<usize>, args: &mut SyscallRegs) {
    // debug!("{}", core::any::type_name_of_val(&f));

    debug!(
        "{}<{}>({}, {}, {}, {}, {})",
        core::any::type_name_of_val(&f),
        args.syscall_id,
        args.arg0,
        args.arg1,
        args.arg2,
        args.arg3,
        args.arg4,
    );

    let res = f(args);
    // debug!(" \\= {res:?}");
    args.syscall_id = Error::encode(res) as u64;
}

fn log(args: &mut SyscallRegs) -> Result<usize> {
    let str = read_untrusted_str(args.arg0, args.arg1)?;
    print!("{str}");
    return Ok(0);
}

fn syscall_todo(_args: &mut SyscallRegs) -> Result<usize> {
    todo!()
}
