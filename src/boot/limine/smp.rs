use limine::{LimineSmpInfo, LimineSmpRequest};
use spin::Lazy;

use crate::{
    debug, error,
    smp::{Cpu, CPU_COUNT},
    smp_main,
};

//

pub fn init() -> Cpu {
    let boot = boot_cpu();

    let mut cpu_count = 1usize;
    for cpu in REQ
        .get_response()
        .get_mut()
        .into_iter()
        .flat_map(|resp| resp.cpus().iter_mut())
        .filter(|cpu| boot.processor_id != cpu.processor_id)
    {
        cpu_count += 1;
        cpu.goto_address = smp_start;
    }

    CPU_COUNT.call_once(|| cpu_count);

    boot
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
                    .map(|cpu| (&**cpu).into())
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

extern "C" fn smp_start(info: *const LimineSmpInfo) -> ! {
    let info = unsafe { &*info };
    smp_main(Cpu::from(info));
}

//

impl From<&LimineSmpInfo> for Cpu {
    fn from(value: &LimineSmpInfo) -> Self {
        Self::new(value.processor_id, value.lapic_id)
    }
}

//

static REQ: LimineSmpRequest = LimineSmpRequest::new(0);
