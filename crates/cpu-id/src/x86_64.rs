use core::sync::atomic::{AtomicUsize, Ordering};

use x86_64::registers::model_specific::Msr;

//

const IA32_TSC_AUX: u32 = 0xC0000103;

//

/// [`cpu_id()`] < [`cpu_count()`]
pub fn cpu_count() -> usize {
    hyperion_boot::cpu_count()
}

/// technically UB to read before a call to [`init`] on this CPU
#[inline(always)]
pub fn cpu_id() -> usize {
    let tsc = Msr::new(IA32_TSC_AUX);
    unsafe { tsc.read() as _ }
}

/// initialize [`cpu_id`]
pub fn init() {
    static CPU_ID_GEN: AtomicUsize = AtomicUsize::new(0);
    let cpu_id = CPU_ID_GEN.fetch_add(1, Ordering::SeqCst);

    if cpu_id >= cpu_count() {
        panic!("generated cpu_id exceeds cpu_count");
    }

    // SAFETY: each cpu gets its own id from the CPU_ID_GEN and the last cpu's
    // id will be lower than `cpu_count`
    unsafe { set_cpu_id(cpu_id) };
}

/// # Safety
///
/// id's should be unique to each CPU
/// and the highest id should not be higher or equal to [`cpu_count`]
unsafe fn set_cpu_id(id: usize) {
    let mut tsc = Msr::new(IA32_TSC_AUX);
    unsafe { tsc.write(id as _) }
}
