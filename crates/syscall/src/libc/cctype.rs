use core::ffi;

//

#[no_mangle]
pub extern "C" fn isdigit(c: ffi::c_int) -> ffi::c_int {
    (b'0' as ffi::c_int..=b'9' as ffi::c_int).contains(&c) as ffi::c_int
}

#[no_mangle]
pub extern "C" fn isspace(c: ffi::c_int) -> ffi::c_int {
    (c == 0x20 || c == 0x0c || c == 0x0a || c == 0x0d || c == 0x09 || c == 0x0b) as ffi::c_int
}

#[no_mangle]
pub extern "C" fn islower(c: ffi::c_int) -> ffi::c_int {
    (b'a' as ffi::c_int..=b'z' as ffi::c_int).contains(&c) as ffi::c_int
}

#[no_mangle]
pub extern "C" fn isupper(c: ffi::c_int) -> ffi::c_int {
    (b'A' as ffi::c_int..=b'Z' as ffi::c_int).contains(&c) as ffi::c_int
}

#[no_mangle]
pub extern "C" fn toupper(c: ffi::c_int) -> ffi::c_int {
    if islower(c) != 0 {
        c & !0x20
    } else {
        c
    }

    // (character as u8).to_ascii_uppercase() as _
    // char::from_u32(character as _);
}
