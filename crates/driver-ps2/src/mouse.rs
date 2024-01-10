use core::sync::atomic::{AtomicBool, Ordering};

use hyperion_driver_acpi::ioapic::IoApic;
use hyperion_interrupts::end_of_interrupt;
use x86_64::instructions::port::Port;

//

pub fn init() {
    static ONCE: AtomicBool = AtomicBool::new(true);
    if ONCE.swap(false, Ordering::SeqCst) {
        hyperion_log::trace!("PS/2 mouse init");

        if let Some(mut io_apic) = IoApic::any() {
            let irq = hyperion_interrupts::set_any_interrupt_handler(
                |irq| irq >= 0x20,
                |irq, ip| {
                    let ps2_byte: u8 = unsafe { Port::new(0x60).read() };

                    hyperion_input::mouse::buffer::send_raw(ps2_byte, ip);

                    end_of_interrupt(irq);
                },
            )
            .expect("No room for PS/2 mouse irq");

            io_apic.set_irq_any(12, irq);
            hyperion_log::debug!("PS/2 mouse irq: {irq}");

            /* unsafe {
                // reset mouse state
                Port::new(0x64).write(0xFFu8);
            } */

            wait_2();
            unsafe {
                // enable aux dev
                Port::new(0x64).write(0xA8u8);
            };

            wait_2();
            unsafe {
                // start reading status
                Port::new(0x64).write(0x20u8);
            }

            wait_1();
            let mut status: u8 = unsafe {
                // read status
                Port::new(0x60).read()
            };

            status |= 1 << 1; // enable IRQ12
            status &= !(1 << 5); // enable mouse clock

            wait_2();
            unsafe {
                // start writing status
                Port::new(0x64).write(0x60u8);
            }

            wait_2();
            unsafe {
                // write status
                Port::new(0x60).write(status);
            }

            wait_2();
            unsafe {
                // start writing default settings
                Port::new(0x64).write(0xD4u8);
            }

            wait_2();
            unsafe {
                // write default settings
                Port::new(0x60).write(0xF6u8);
            }

            wait_2();
            unsafe {
                // start enabling mouse
                Port::new(0x64).write(0xD4u8);
            }

            wait_2();
            unsafe {
                // enable mouse
                Port::new(0x60).write(0xF4u8);
            }

            wait_2();
            unsafe {
                // start writing sample rate
                Port::new(0x64).write(0xF3u8);
            }

            wait_2();
            unsafe {
                // write sample rate
                Port::new(0x60).write(80u8);
            }
        }
    }
}

//

fn wait_1() {
    for _ in 0..100_000 {
        if unsafe { Port::<u8>::new(0x64).read() & 1 == 1 } {
            break;
        }
    }
}

fn wait_2() {
    for _ in 0..100_000 {
        if unsafe { Port::<u8>::new(0x64).read() & 2 == 0 } {
            break;
        }
    }
}
