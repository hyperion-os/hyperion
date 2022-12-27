use core::arch::asm;

//

/// Halts the CPU and never returns
pub fn hlt() -> ! {
    loop {
        unsafe { asm!("hlt") }
    }
}
