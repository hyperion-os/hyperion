use core::{
    mem::transmute,
    sync::atomic::{AtomicUsize, Ordering},
};

use hyperion_boot_interface::Cpu;
use hyperion_log::{debug, error};
use limine::SmpRequest;
use spin::{Lazy, Once};

//

pub fn smp_init(i_am_bsp: bool, start: extern "C" fn() -> !) {
    let boot = boot_cpu();
    let cpu_count = cpu_count();

    // each cpu wakes up the next 2 CPUs to snowball the CPU waking
    static NEXT_CPU_TO_WAKE: AtomicUsize = AtomicUsize::new(0);

    let mut next;
    while {
        next = NEXT_CPU_TO_WAKE.fetch_add(1, Ordering::Relaxed);
        next < cpu_count
    } {
        let did_start_ap = wake_nth(next, boot.processor_id, start);

        if did_start_ap && i_am_bsp {
            // bsp can go do something else while the APs are snowballing
            return;
        }
    }
}

/// returns `true` only if an AP was started
fn wake_nth(n: usize, bsp_id: u32, start: extern "C" fn() -> !) -> bool {
    if let Some(resp) = REQ.get_response().get_mut() {
        let cpu = &mut resp.cpus()[n];
        if cpu.processor_id == bsp_id {
            return false;
        }

        // SAFETY: afaik it is safe to transmute one of the arguments away,
        // it is in a specific register and gets ignored
        cpu.goto_address = unsafe {
            transmute::<extern "C" fn() -> !, extern "C" fn(*const limine::SmpInfo) -> !>(start)
        };

        return true;
    }

    false
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

static REQ: SmpRequest = SmpRequest::new(0);
