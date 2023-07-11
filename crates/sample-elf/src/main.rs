#![no_std]
#![no_main]
#![feature(lang_items)]

#[no_mangle]
pub extern "C" fn _start() -> ! {
    /* // page fault test:
    let null_ptr = core::hint::black_box(0x0) as *const u8;
    core::hint::black_box(unsafe { *null_ptr }); */

    loop {
        trigger_syscall(42, 43, 44, 0, 45, 46);
    }
}

pub fn main(args: &[&str]) -> Result<(), i64> {
    Ok(())
}

pub fn trigger_old_syscall(syscall_num: u32) {
    unsafe {
        core::arch::asm!("int 0xAA", in("eax") syscall_num);
    }
}

pub extern "C" fn trigger_syscall(
    _rdi: u64,
    _rsi: u64,
    _rdx: u64,
    _rcx_ignored: u64,
    _r8: u64,
    _r9: u64,
) {
    unsafe {
        core::arch::asm!("syscall");
    }
}

#[panic_handler]
fn panic_handler(_info: &core::panic::PanicInfo) -> ! {
    loop {}
}

// for clippy:
#[lang = "eh_personality"]
extern "C" fn eh_personality() {}
