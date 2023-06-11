use crossbeam::atomic::AtomicCell;
use hyperion_boot_interface::Cpu;
use hyperion_log::{debug, error};
use limine::{LimineSmpInfo, LimineSmpRequest};
use spin::Lazy;

//

pub fn smp_init(dest: fn(Cpu) -> !) -> ! {
    SMP_DEST.store(dest);

    let boot = boot_cpu();

    for cpu in REQ
        .get_response()
        .get_mut()
        .into_iter()
        .flat_map(|resp| resp.cpus().iter_mut())
        .filter(|cpu| boot.processor_id != cpu.processor_id)
    {
        cpu.goto_address = smp_start;
    }

    dest(boot);
}

pub fn cpu_count() -> usize {
    REQ.get_response().get_mut().unwrap().cpu_count as usize
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

extern "C" fn smp_start(info: *const LimineSmpInfo) -> ! {
    let info = unsafe { &*info };
    SMP_DEST.load()(Cpu::new(info.processor_id, info.lapic_id));
}

//

static SMP_DEST: AtomicCell<fn(Cpu) -> !> = AtomicCell::new(|_| unreachable!());
static REQ: LimineSmpRequest = LimineSmpRequest::new(0);
