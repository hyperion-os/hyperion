#![no_std]
#![no_main]

#[panic_handler]
fn panic_handler(_: &core::panic::PanicInfo) -> ! {
    loop {}
}

#[no_mangle]
#[link_section = ".boot"]
pub extern "C" fn kernel_main() -> ! {
    unsafe {
        *(0xB8000 as *mut u32) = 0x4f524f45;
    }

    loop {
        unsafe {
            core::arch::asm!("hlt");
        }
    }
}
