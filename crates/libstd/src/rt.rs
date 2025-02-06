use core::{
    arch::naked_asm,
    ptr::{self, NonNull},
};

use hyperion_syscall::{fs::FileDesc, mem_map, MemMapFlags};

use crate::process::{ExitCode, Termination};

//

fn lang_start_internal<T: Termination>(main: fn() -> T) -> isize {
    main().report().to_i32() as _
}

#[lang = "start"]
fn lang_start<T: Termination>(
    main: fn() -> T,
    _argc: isize,
    _argv: *const *const u8,
    _idk: u8,
) -> isize {
    lang_start_internal(main)
}

pub(crate) const USER_SPACE_TOP: usize = 0x8000_0000_0000;
pub(crate) const MAIN_STACK_TOP: usize = USER_SPACE_TOP;
pub(crate) const STACK_GUARD_SIZE: usize = 0x20_0000; // 2 MiB stack guard pages
pub(crate) const MAIN_STACK_SIZE: usize = 0x200_0000 - STACK_GUARD_SIZE; // 30 MiB main thread stack
pub(crate) const MAIN_STACK_BOTTOM: usize = USER_SPACE_TOP - MAIN_STACK_SIZE;
pub(crate) const MAIN_STACK_GUARD_BOTTOM: usize =
    USER_SPACE_TOP - MAIN_STACK_SIZE - STACK_GUARD_SIZE;

#[no_mangle]
#[naked]
extern "C" fn _start() -> ! {
    unsafe {
        naked_asm!(
            "mov rax, {mem_map}",
            "mov rdi, {main_thread_stack}",
            "mov rsi, {main_thread_stack_len}",
            "mov rdx, {map_flags}",
            "mov r8, 0",
            "mov r9, 0",
            "syscall",

            "mov rsp, rax",
            "jmp _start_with_stack",
            mem_map = const crate::sys::id::MEM_MAP,
            main_thread_stack = const MAIN_STACK_GUARD_BOTTOM,
            main_thread_stack_len = const MAIN_STACK_SIZE + STACK_GUARD_SIZE,
            map_flags = const crate::sys::MemMapFlags::RW.bits() | crate::sys::MemMapFlags::ANON.bits(),
        );
    }
}

#[no_mangle]
extern "C" fn _start_with_stack() -> ! {
    // insert the guard page
    mem_map(
        NonNull::new(MAIN_STACK_GUARD_BOTTOM as _),
        STACK_GUARD_SIZE,
        MemMapFlags::FIXED | MemMapFlags::ANON,
        FileDesc(0),
        0,
    )
    .expect("failed to insert main thread guard page");

    // init cli args from stack, move them to the heap
    // crate::println!("init cli args");
    // unsafe { env::init_args(hyperion_cli_args_ptr) };

    // call `lang_start`
    // crate::println!("calling main");
    let exit_code = unsafe { main(0, ptr::null()) };
    // crate::println!("exit:{exit_code}");

    ExitCode::from_raw(exit_code as _).exit_process();
}

// rustc generates the real `main` function, that fn
// simply calls `lang_start` with the correct args
extern "Rust" {
    fn main(argc: isize, argv: *const *const u8) -> isize;
}
