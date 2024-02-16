use core::{ffi, iter, ptr, slice, str::from_utf8};

//

#[no_mangle]
unsafe extern "C" fn strcmp(lhs: *const ffi::c_char, rhs: *const ffi::c_char) -> ffi::c_int {
    unsafe { strncmp(lhs, rhs, usize::MAX) }
}

#[no_mangle]
unsafe extern "C" fn strncmp(
    lhs: *const ffi::c_char,
    rhs: *const ffi::c_char,
    num: usize,
) -> ffi::c_int {
    let lhs = unsafe { c_str_iter(lhs) };
    let rhs = unsafe { c_str_iter(rhs) };

    for (l, r) in lhs.zip(rhs).take(num) {
        if l != r || l == 0 {
            return l as ffi::c_int - r as ffi::c_int;
        }
    }

    0
}

fn _strncmp_assert(lhs: &str, rhs: &str, n: usize, expected: i32) {
    let val = unsafe { strncmp(lhs.as_ptr() as _, rhs.as_ptr() as _, n) }.signum();
    assert_eq!(
        val, expected,
        "strncmp({lhs}, {rhs}, {n}) => {val}, expected: {expected}"
    );
}

fn _strncmp_test() {
    _strncmp_assert("a\0", "a\0", usize::MAX, 0);
    _strncmp_assert("a\0", "a1\0", usize::MAX, -1);
    _strncmp_assert("a1\0", "a\0", usize::MAX, 1);
    _strncmp_assert("\0", "\0", usize::MAX, 0);
    _strncmp_assert("test", "test", 4, 0);
    _strncmp_assert("test1", "test2", 5, -1);
}

#[no_mangle]
unsafe extern "C" fn strcasecmp(lhs: *const ffi::c_char, rhs: *const ffi::c_char) -> ffi::c_int {
    unsafe { strncasecmp(lhs, rhs, usize::MAX) }
}

#[no_mangle]
unsafe extern "C" fn strncasecmp(
    lhs: *const ffi::c_char,
    rhs: *const ffi::c_char,
    num: usize,
) -> ffi::c_int {
    let lhs = unsafe { c_str_iter(lhs) };
    let rhs = unsafe { c_str_iter(rhs) };

    for (l, r) in lhs.zip(rhs).take(num) {
        let l = (l as u8).to_ascii_lowercase() as ffi::c_int;
        let r = (r as u8).to_ascii_lowercase() as ffi::c_int;

        if l != r || l == 0 {
            return l - r;
        }
    }

    0
}

fn _strncasecmp_assert(lhs: &str, rhs: &str, n: usize, expected: i32) {
    let val = unsafe { strncasecmp(lhs.as_ptr() as _, rhs.as_ptr() as _, n) }.signum();
    assert_eq!(
        val, expected,
        "strncasecmp({lhs}, {rhs}, {n}) => {val}, expected: {expected}"
    );
}

fn _strncasecmp_test() {
    _strncasecmp_assert("a\0", "a\0", usize::MAX, 0);
    _strncasecmp_assert("a\0", "A\0", usize::MAX, 0);
    _strncasecmp_assert("a\0", "a1\0", usize::MAX, -1);
    _strncasecmp_assert("a\0", "A1\0", usize::MAX, -1);
    _strncasecmp_assert("a1\0", "a\0", usize::MAX, 1);
    _strncasecmp_assert("\0", "\0", usize::MAX, 0);
    _strncasecmp_assert("test", "test", 4, 0);
    _strncasecmp_assert("teSt", "tEsT", 4, 0);
    _strncasecmp_assert("test1", "test2", 5, -1);
    _strncasecmp_assert("test1", "Test2", 5, -1);
    _strncasecmp_assert("test", "TEST", 4, 0);
    _strncasecmp_assert("test", "yeet", 0, 0);
}

// iterate all chars in a c string including the null terminator
pub(super) unsafe fn c_str_iter(mut str: *const ffi::c_char) -> impl Iterator<Item = ffi::c_char> {
    iter::from_fn(move || {
        let c = unsafe { *str };
        str = unsafe { str.byte_add(1) };
        (c != 0).then_some(c)
    })
    .chain([0])
}

#[no_mangle]
unsafe extern "C" fn strchr(str: *const ffi::c_char, character: ffi::c_int) -> *const ffi::c_char {
    let character = character as ffi::c_char;

    for (i, c) in unsafe { c_str_iter(str) }.enumerate() {
        if c == character {
            return unsafe { str.add(i) };
        }
    }

    ptr::null_mut()
}

#[no_mangle]
extern "C" fn strrchr() {
    unimplemented!()
}

#[no_mangle]
unsafe extern "C" fn strncpy(
    dst: *mut ffi::c_char,
    src: *const ffi::c_char,
    num: usize,
) -> *mut ffi::c_char {
    let mut i = 0;

    while unsafe { *src.add(i) } != 0 && i < num {
        unsafe { *dst.add(i) = *src.add(i) };
        i += 1;
    }

    for i in i..num {
        unsafe { *dst.add(i) = 0 };
    }

    dst
}

#[no_mangle]
unsafe extern "C" fn strlen(str: *const ffi::c_char) -> usize {
    unsafe { strnlen(str, usize::MAX) }
}

#[no_mangle]
unsafe extern "C" fn strnlen(str: *const ffi::c_char, size: usize) -> usize {
    unsafe { c_str_iter(str).take(size).take_while(|&c| c != 0).count() }
}

fn _strlen_assert(lhs: &str, expected: usize) {
    let val = unsafe { strlen(lhs.as_ptr() as *const ffi::c_char) };
    assert_eq!(
        val, expected,
        "strlen({lhs}) => {val}, expected: {expected}"
    );
}

fn _strnlen_assert(lhs: &str, n: usize, expected: usize) {
    let val = unsafe { strnlen(lhs.as_ptr() as *const ffi::c_char, n) };
    assert_eq!(
        val, expected,
        "strnlen({lhs}, {n}) => {val}, expected: {expected}"
    );
}

fn _strlen_test() {
    _strlen_assert("\0", 0);
    _strlen_assert("  \0", 2);
    _strlen_assert("  1\0", 3);
    _strlen_assert("  1  \0", 5);
    _strlen_assert("  654  \0", 7);
    _strlen_assert(" 3d\0", 3);
    _strlen_assert(" 3d\0", 3);

    _strnlen_assert("  654  ", 7, 7);
    _strnlen_assert("  654  ", 4, 4);
    _strnlen_assert("  654  ", 0, 0);
    _strnlen_assert("  \054  ", 7, 2);
}

// void* memcpy( void *dest, const void *src, size_t count );
#[no_mangle]
unsafe extern "C" fn memcpy(dest: *mut ffi::c_void, src: *const ffi::c_void, count: usize) {
    let mut dest = dest.cast::<u8>();
    let mut src = src.cast::<u8>();

    for _ in 0..count {
        unsafe {
            *dest = *src;
            dest = dest.add(1);
            src = src.add(1);
        }
    }
}

// int memcmp( const void* lhs, const void* rhs, usize count );
#[no_mangle]
unsafe extern "C" fn memcmp(
    lhs: *const ffi::c_void,
    rhs: *const ffi::c_void,
    count: usize,
) -> ffi::c_int {
    let lhs = lhs.cast::<u8>();
    let rhs = rhs.cast::<u8>();

    for i in 0..count {
        let l = unsafe { *lhs.add(i) };
        let r = unsafe { *rhs.add(i) };

        if l != r {
            return l as ffi::c_int - r as ffi::c_int;
        }
    }

    0
}

// void* memmove( void* dest, const void* src, usize count );
#[no_mangle]
unsafe extern "C" fn memmove(
    dest: *mut ffi::c_void,
    src: *const ffi::c_void,
    count: usize,
) -> *mut ffi::c_void {
    let dest = dest.cast::<u8>();
    let src = src.cast::<u8>();

    if dest.cast_const() < src {
        for i in 0..count {
            unsafe { *dest.add(i) = *src.add(i) };
        }
    } else {
        for i in (0..count).rev() {
            unsafe { *dest.add(i) = *src.add(i) };
        }
    }

    dest.cast()
}

// void* memset( void* dest, int ch, std::usize count );
#[no_mangle]
unsafe extern "C" fn memset(dest: *mut ffi::c_void, ch: ffi::c_int, count: usize) {
    let dest = dest.cast::<u8>();

    for i in 0..count {
        unsafe { *dest.add(i) = ch as u8 }
    }
}

//

#[allow(unused)]
pub(super) unsafe fn as_rust_str<'a>(str: *const ffi::c_char) -> Option<&'a str> {
    let len = unsafe { strlen(str) };

    let str = unsafe { slice::from_raw_parts(str as *const u8, len) };
    match from_utf8(str) {
        Ok(s) => Some(s),
        Err(_) => None,
    }
}
