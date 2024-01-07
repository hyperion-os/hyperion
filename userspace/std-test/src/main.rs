#![feature(c_size_t)]

use core::ffi;

// void* memcpy( void *dest, const void *src, size_t count );
#[no_mangle]
pub extern "C" fn memcpy(dest: *mut ffi::c_void, src: *const ffi::c_void, count: usize) {
    panic!("memcpy {dest:?} {src:?} {count}");
}

// int memcmp( const void* lhs, const void* rhs, size_t count );
#[no_mangle]
pub extern "C" fn memcmp(
    lhs: *const ffi::c_void,
    rhs: *const ffi::c_void,
    count: ffi::c_size_t,
) -> std::ffi::c_int {
    panic!("memcmp {lhs:?} {rhs:?} {count}");
}

// void* memmove( void* dest, const void* src, size_t count );
#[no_mangle]
pub extern "C" fn memmove(
    dest: *mut ffi::c_void,
    src: *const ffi::c_void,
    count: usize,
) -> *mut ffi::c_void {
    panic!("memmove {dest:?} {src:?} {count}");
}

#[no_mangle]
pub extern "C" fn __libc_start_main() -> ! {
    panic!();
}

fn main() {
    println!("Hello, world!");
}
