#![no_std]
#![no_main]

pub mod vga;

#[panic_handler]
fn panic_handler(_: &core::panic::PanicInfo) -> ! {
    loop {}
}

#[no_mangle]
#[link_section = ".boot"]
pub extern "C" fn kernel_main(magic_num: u64) -> ! {
    // null byte clears the VGA buffer
    print!("\0");
    println!("Hello from Hyperion, magic_num = {magic_num}");

    loop {
        unsafe {
            core::arch::asm!("hlt");
        }
    }
}
