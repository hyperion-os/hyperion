use core::sync::atomic::{AtomicBool, Ordering};

use hyperion_driver_acpi::ioapic::IoApic;
use hyperion_interrupts::end_of_interrupt;
use x86_64::instructions::port::Port;

//

pub fn init() {
    static ONCE: AtomicBool = AtomicBool::new(true);
    if ONCE.swap(false, Ordering::SeqCst) {
        hyperion_log::trace!("PS/2 keyboard init");

        if let Some(mut io_apic) = IoApic::any() {
            let irq = hyperion_interrupts::set_any_interrupt_handler(
                |irq| irq >= 0x20,
                |irq| {
                    let ps2_byte: u8 = unsafe { Port::new(0x60).read() };

                    hyperion_keyboard::provide_keyboard_event(ps2_byte);
                    end_of_interrupt(irq);
                },
            )
            .expect("No room for PS/2 keyboard irq");

            io_apic.set_irq_any(1, irq);
            hyperion_log::debug!("PS/2 keyboard irq: {irq}");
        }
    }
}
