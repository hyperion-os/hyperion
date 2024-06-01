#![no_std]

//

use core::arch::asm;

//

/// HCF instruction
pub fn halt_and_catch_fire() -> ! {
    loop {
        wait_for_interrupts();
    }
}

/// WFI instruction
pub extern "C" fn wait_for_interrupts() {
    unsafe {
        asm!("wfi");
    }
}
