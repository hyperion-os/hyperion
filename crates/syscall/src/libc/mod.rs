//! some libc things that Rust expects

mod cctype;
mod cstdlib;
mod cstring;

//

#[no_mangle]
extern "C" fn __stack_chk_fail() {
    unimplemented!()
}

#[no_mangle]
extern "C" fn __libc_start_main() {}
