use core::ptr;

use crate::{
    env,
    process::{ExitCode, Termination},
};

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

#[no_mangle]
#[naked]
extern "C" fn _start() -> ! {
    unsafe { core::arch::asm!("jmp rust_start", options(noreturn)) }
}

#[no_mangle]
extern "C" fn rust_start(hyperion_cli_args_ptr: usize, _a2: usize) -> ! {
    // rustc generates the real `main` function, that fn
    // simply calls `lang_start` with the correct args
    extern "Rust" {
        fn main(argc: isize, argv: *const *const u8) -> isize;
    }

    // init cli args from stack, move them to the heap
    // crate::println!("init cli args");
    unsafe { env::init_args(hyperion_cli_args_ptr) };

    // call `lang_start`
    // crate::println!("calling main");
    let exit_code = unsafe { main(0, ptr::null()) };
    // crate::println!("exit:{exit_code}");

    ExitCode::from_raw(exit_code as _).exit_process();
}
