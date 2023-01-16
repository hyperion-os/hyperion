use crate::arch;

//

pub use mem::memmap;
pub use term::_print;

//

mod cmdline;
mod framebuffer;
mod mem;
mod smp;
mod term;

//

#[no_mangle]
pub extern "C" fn _start() -> ! {
    crate::BOOTLOADER.call_once(|| "Limine");

    framebuffer::init();
    cmdline::init();

    arch::early_boot_cpu();
    arch::early_per_cpu();

    crate::kernel_main()
}

pub fn smp_init() {
    smp::init();
}
