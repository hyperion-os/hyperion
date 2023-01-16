use crate::{
    arch,
    smp::{smp_main, Cpu},
};
use limine::{LimineSmpInfo, LimineSmpRequest};

//

pub fn init() -> Cpu {
    static REQ: LimineSmpRequest = LimineSmpRequest::new(0);

    let mut boot = Cpu::new(0, 0);

    for cpu in REQ
        .get_response()
        .get_mut()
        .into_iter()
        .flat_map(|resp| {
            let bsp_lapic_id = resp.bsp_lapic_id;
            resp.cpus().iter_mut().map(move |cpu| (bsp_lapic_id, cpu))
        })
        .filter_map(|(bsp_lapic_id, cpu)| {
            if bsp_lapic_id == cpu.lapic_id {
                boot = Cpu::from(&**cpu);
                None
            } else {
                Some(cpu)
            }
        })
    {
        cpu.goto_address = smp_start;
    }

    boot
}

extern "C" fn smp_start(info: *const LimineSmpInfo) -> ! {
    let info = unsafe { &*info };
    arch::early_per_cpu();
    smp_main(Cpu::from(info));
}

//

impl From<&LimineSmpInfo> for Cpu {
    fn from(value: &LimineSmpInfo) -> Self {
        Self::new(value.processor_id, value.lapic_id)
    }
}
