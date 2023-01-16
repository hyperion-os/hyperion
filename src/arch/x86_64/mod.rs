use crate::debug;

//

pub mod gdt;
pub mod idt;

//

pub fn early_boot_cpu() {
    gdt::init();
    idt::init();

    debug!("Re-enabling x86_64 interrupts");
    x86_64::instructions::interrupts::enable();
}

pub fn early_per_cpu() {}

pub fn done() -> ! {
    loop {
        x86_64::instructions::hlt();
    }
}
