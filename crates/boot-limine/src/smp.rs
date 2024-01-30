use core::mem::transmute;

use hyperion_boot_interface::Cpu;
use hyperion_log::{debug, error};
use limine::SmpRequest;
use spin::{Lazy, Once};

//

pub fn smp_init() {
    let boot = boot_cpu();

    for cpu in REQ
        .get_response()
        .get_mut()
        .into_iter()
        .flat_map(|resp| resp.cpus().iter_mut())
        .filter(|cpu| boot.processor_id != cpu.processor_id)
    {
        cpu.goto_address = unsafe { transmute(_start as usize) };
    }
}

pub fn cpu_count() -> usize {
    static CPU_COUNT: Once<usize> = Once::new();
    *CPU_COUNT.call_once(|| REQ.get_response().get_mut().unwrap().cpu_count as usize)
}

pub fn boot_cpu() -> Cpu {
    static BOOT_CPU: Lazy<Cpu> = Lazy::new(|| {
        let boot = REQ
            .get_response()
            .get_mut()
            .and_then(|resp| {
                let bsp_lapic_id = resp.bsp_lapic_id;
                resp.cpus()
                    .iter_mut()
                    .find(move |cpu| bsp_lapic_id == cpu.lapic_id)
                    .map(|cpu| Cpu::new(cpu.processor_id, cpu.lapic_id))
            })
            .unwrap_or_else(|| {
                error!("Boot CPU not found");
                Cpu::new(0, 0)
            });

        debug!("Boot CPU is {boot:#}");

        boot
    });

    *BOOT_CPU
}

pub fn lapics() -> impl Iterator<Item = u32> {
    REQ.get_response()
        .get_mut()
        .into_iter()
        .flat_map(|resp| resp.cpus().iter())
        .map(|cpu| cpu.lapic_id)
}

//

extern "C" {
    fn _start() -> !;
}

//

static REQ: SmpRequest = SmpRequest::new(0);
