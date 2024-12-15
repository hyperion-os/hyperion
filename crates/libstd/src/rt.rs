use core::{arch::naked_asm, mem::MaybeUninit, ptr};

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

#[allow(dead_code)]
#[repr(align(0x1000))]
struct Page([u8; 0x1000]);

static mut MAIN_THREAD_STACK: MaybeUninit<[Page; 8]> = MaybeUninit::zeroed();

#[no_mangle]
#[naked]
extern "C" fn _start() -> ! {
    unsafe {
        naked_asm!(
            "lea rsp, {main_thread_stack} + 0x8000",
            "jmp _start_with_stack",
            main_thread_stack = sym MAIN_THREAD_STACK,
        );
    }
}

#[no_mangle]
extern "C" fn _start_with_stack() -> ! {
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
