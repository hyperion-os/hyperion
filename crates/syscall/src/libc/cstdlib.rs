use core::ffi;

use super::{
    cctype::{isdigit, isspace},
    cstring::c_str_iter,
};

//

#[no_mangle]
extern "C" fn abs(n: ffi::c_int) -> ffi::c_int {
    n.abs()
}

#[no_mangle]
unsafe extern "C" fn atoi(str: *const ffi::c_char) -> ffi::c_int {
    let mut iter = unsafe { c_str_iter(str) }
        .skip_while(|&c| isspace(c as _) != 0)
        .peekable();

    let mut neg = false;
    match iter.peek().unwrap() {
        0x2d => {
            // b'-'
            iter.next();
            neg = true;
        }
        0x2b => {
            // b'+'
            iter.next();
        }
        _ => {}
    }

    let mut res = 0;
    for digit in iter.take_while(|&c| isdigit(c as _) != 0) {
        res = 10 * res + b'0' as ffi::c_int - digit as ffi::c_int;
    }

    if neg {
        res
    } else {
        -res
    }

    // let Some(str) = as_rust_str(str) else {
    //     return 0;
    // };

    // let str = str.trim().trim_start_matches(|c| c == '+');
    // if str.is_empty() {
    //     return 0;
    // }

    // let str = str
    //     .find(|c: char| !c.is_digit(10))
    //     .and_then(|last| str.get(..last))
    //     .unwrap_or(str);

    // str.parse().unwrap()
}

fn _atoi_assert(lhs: &str, expected: i32) {
    let val = unsafe { atoi(lhs.as_ptr() as *const ffi::c_char) };
    assert_eq!(val, expected, "atoi({lhs}) => {val}, expected: {expected}");
}

fn _atoi_test() {
    _atoi_assert("\0", 0);
    _atoi_assert("  \0", 0);
    _atoi_assert("  1\0", 1);
    _atoi_assert("  1  \0", 1);
    _atoi_assert("  654  \0", 654);
    _atoi_assert("  654  ", 654);
    _atoi_assert(" 3d\0", 3);
    _atoi_assert("-3d\0", -3);
    _atoi_assert("a-3d\0", 0);
    _atoi_assert("+3d\0", 3);
}
