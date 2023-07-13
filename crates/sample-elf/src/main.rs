#![no_std]
#![no_main]
#![feature(lang_items)]

#[no_mangle]
pub extern "C" fn _start() -> ! {
    /* // page fault test:
    let null_ptr = core::hint::black_box(0x0) as *const u8;
    core::hint::black_box(unsafe { *null_ptr }); */

    hyperion_syscall::log("testing");

    loop {}
}

pub fn main(args: &[&str]) -> Result<(), i64> {
    Ok(())
}

#[panic_handler]
fn panic_handler(_info: &core::panic::PanicInfo) -> ! {
    loop {}
}

// for clippy:
#[lang = "eh_personality"]
extern "C" fn eh_personality() {}
