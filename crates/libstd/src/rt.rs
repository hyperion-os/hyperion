use core::mem;

use crate::env;

//

fn lang_start_internal<T>(main: fn() -> T, a0: usize, _a1: usize, _a2: usize) -> isize {
    unsafe { env::init_args(a0) };
    main();
    0
}

#[lang = "start"]
fn lang_start<T>(main: fn() -> T, argc: isize, argv: *const *const u8, idk: u8) -> isize {
    lang_start_internal(
        main,
        unsafe { mem::transmute::<isize, usize>(argc) },
        argv as _,
        idk as _,
    )
}
