use core::sync::atomic::{AtomicBool, AtomicI16, AtomicI8, Ordering};

use crossbeam::atomic::AtomicCell;
use hyperion_driver_acpi::ioapic::IoApic;
use x86_64::instructions::port::Port;

//

pub fn init() {
    static ONCE: AtomicBool = AtomicBool::new(true);
    if ONCE.swap(false, Ordering::SeqCst) {
        hyperion_log::trace!("PS/2 mouse init");

        if let Some(mut io_apic) = IoApic::any() {
            let irq = hyperion_interrupts::set_any_interrupt_handler(
                |irq| irq >= 0x20,
                || {
                    /* hyperion_log::debug!(
                        "avail?: {}",
                        unsafe { Port::<u8>::new(0x64).read() } & 0b1
                    ); */
                    let data: u8 = unsafe { Port::new(0x60).read() };
                    let data: i8 = data as _;

                    match NEXT.load() {
                        MouseData::SomethingIdk => {
                            DATA.0.store(data, Ordering::Release);
                            NEXT.store(MouseData::X);
                        }
                        MouseData::X => {
                            DATA.1.store(data, Ordering::Release);
                            NEXT.store(MouseData::Y);
                        }
                        MouseData::Y => {
                            let _cmd: i16 = DATA.0.load(Ordering::Acquire) as _;
                            let x: i16 = DATA.1.load(Ordering::Acquire) as _;
                            let y: i16 = data as _;
                            let _x = MOUSE.0.fetch_add(x, Ordering::Release);
                            let _y = MOUSE.1.fetch_add(y, Ordering::Release);
                            NEXT.store(MouseData::SomethingIdk);

                            // TODO: provide mouse event
                        }
                    }
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

// these shouldn't be touched from multiple threads or interrupts inside interrupts
static NEXT: AtomicCell<MouseData> = AtomicCell::new(MouseData::X);
static DATA: (AtomicI8, AtomicI8) = (AtomicI8::new(0), AtomicI8::new(0));
static MOUSE: (AtomicI16, AtomicI16) = (AtomicI16::new(0), AtomicI16::new(0));

const _: () = assert!(AtomicCell::<MouseData>::is_lock_free());

//

#[derive(Debug, Clone, Copy)]
#[repr(u8)]
enum MouseData {
    SomethingIdk,
    X,
    Y,
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
