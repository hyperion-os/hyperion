#![doc = include_str!("../../../README.md")]
//
#![no_std]
#![no_main]
//
#![feature(
    const_option,
    abi_x86_interrupt,
    allocator_api,
    pointer_is_aligned,
    int_roundings,
    array_chunks,
    cfg_target_has_atomic,
    core_intrinsics,
    custom_test_frameworks,
    panic_can_unwind,
    lang_items
)]
#![test_runner(crate::testfw::test_runner)]
#![reexport_test_harness_main = "test_main"]

//

use core::sync::atomic::{AtomicBool, Ordering};

use chrono::Duration;
use futures_util::StreamExt;
use hyperion_boot::{args, hhdm_offset, phys_addr, stack, virt_addr};
use hyperion_boot_interface::{boot, Cpu};
use hyperion_color::Color;
use hyperion_framebuffer::framebuffer::Framebuffer;
use hyperion_kernel_info::{NAME, VERSION};
use hyperion_log::{debug, warn};
use hyperion_mem::from_higher_half;
use hyperion_num_postfix::NumberPostfix;
use hyperion_scheduler::timer::ticks;
use x86_64::{instructions::port::Port, VirtAddr};

use self::arch::rng_seed;
use crate::driver::acpi::ioapic::IoApic;

extern crate alloc;

//

#[path = "./arch/x86_64/mod.rs"]
pub mod arch;
pub mod backtrace;
pub mod driver;
pub mod panic;
#[cfg(test)]
pub mod testfw;

//

#[no_mangle]
fn kernel_main() -> ! {
    hyperion_log_multi::init_logger();

    debug!("Entering kernel_main");

    arch::early_boot_cpu();

    driver::lazy_install();

    debug!("Cmdline: {:?}", args::get());

    debug!(
        "Kernel addr: {:?} ({}B), {:?} ({}B), ",
        virt_addr(),
        virt_addr().postfix_binary(),
        phys_addr(),
        phys_addr().postfix_binary(),
    );
    debug!("HHDM Offset: {:#0X?}", hhdm_offset());
    debug!(
        "Kernel Stack: {:#0X?}",
        from_higher_half(VirtAddr::new(stack().start as u64))
    );

    debug!("{NAME} {VERSION} was booted with {}", boot().name());

    #[cfg(test)]
    test_main();

    // main task(s)
    hyperion_scheduler::spawn(hyperion_kshell::kshell());
    hyperion_scheduler::spawn(spinner());

    // jumps to [smp_main] right bellow + wakes up other threads to jump there
    boot().smp_init(smp_main);
}

fn smp_main(cpu: Cpu) -> ! {
    debug!("{cpu} entering smp_main");

    arch::early_per_cpu(&cpu);

    static KB_ONCE: AtomicBool = AtomicBool::new(true);
    if KB_ONCE.swap(false, Ordering::SeqCst) {
        // code after every CPU and APIC has been initialized
        if let Some(mut io_apic) = IoApic::any() {
            hyperion_interrupts::set_interrupt_handler(33, || {
                let scancode: u8 = unsafe { Port::new(0x60).read() };
                driver::ps2::keyboard::process(scancode);

                /* if driver::ps2::keyboard::debug_key() {
                    unsafe { backtrace::print_backtrace_from(f.stack_pointer) };
                } */
            });

            io_apic.set_irq_any(1, 33);
            debug!("keyboard initialized");
        }
    }

    hyperion_scheduler::run_tasks();
}

async fn spinner() {
    let mut ticks = ticks(Duration::milliseconds(100));

    while ticks.next().await.is_some() {
        let Some( fbo) = Framebuffer::get() else {
            warn!("failed to get fbo");
            break;
        };
        let mut fbo = fbo.lock();

        let r = (rng_seed() % 0xFF) as u8;
        let g = (rng_seed() % 0xFF) as u8;
        let b = (rng_seed() % 0xFF) as u8;
        let x = fbo.width - 60;
        let y = fbo.height - 60;
        fbo.fill(x, y, 50, 50, Color::new(r, g, b));
    }
}

#[lang = "eh_personality"]
extern "C" fn eh_personality() {}
