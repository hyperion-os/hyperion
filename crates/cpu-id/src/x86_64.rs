use core::{
    arch::asm,
    sync::atomic::{AtomicU8, AtomicUsize, Ordering},
};

use x86_64::registers::model_specific::Msr;

//

const IA32_TSC_AUX: u32 = 0xC0000103;
static CPU_ID_DYN: AtomicU8 = AtomicU8::new(0);

//

/// [`cpu_id()`] < [`cpu_count()`]
pub fn cpu_count() -> usize {
    hyperion_boot::cpu_count()
}

// TODO: the return type is not usize, its u32
/// technically UB to read before a call to [`init`] on this CPU
#[inline(always)]
pub extern "C" fn cpu_id() -> usize {
    _cpu_id_dyn()
    // _cpu_id_rdpid()
    // _cpu_id_rdtscp()
    // _cpu_id_tsc_msr()
}

pub fn cpu_id_dyn_type() -> u8 {
    CPU_ID_DYN.load(Ordering::Relaxed)
}

/// 5M cpu_id calls in 47ms141µs160ns (on my system, uses rdtscp)
#[inline(always)]
fn _cpu_id_dyn() -> usize {
    match CPU_ID_DYN.load(Ordering::Relaxed) {
        1 => _cpu_id_rdpid(),
        2 => _cpu_id_rdtscp(),
        3 => _cpu_id_tsc_msr(),
        _ => unreachable!(),
    }
}

fn select_cpu_id_dyn() {
    let val;
    if unsafe { core::arch::x86_64::__cpuid(0x7) }.ecx & (1 << 22) != 0 {
        // rdpid support
        //
        // rdpid is the fastest??? (I cannot test it yet)
        // it only reads the IA32_TSC_AUX into any register
        val = 1;
    } else if unsafe { core::arch::x86_64::__cpuid(0x80000001) }.edx & (1 << 27) != 0 {
        // rdtscp support
        //
        // tdtscp is alot faster than rdmsr
        // but it reads the timestamp counter for no reason
        val = 2;
    } else {
        // at least rdmsr support is expected, processor identification
        // would require APIC or gs or something otherwise
        val = 3;
    };

    CPU_ID_DYN.store(val, Ordering::Relaxed);
}

/// not supported on my system
#[inline(always)]
fn _cpu_id_rdpid() -> usize {
    let cpu_id: usize;
    unsafe {
        asm!("rdpid {x}", x = out(reg) cpu_id);
    }
    cpu_id
}

/// 5M cpu_id calls in 45ms973µs460ns (on my system)
#[inline(always)]
fn _cpu_id_rdtscp() -> usize {
    let cpu_id: usize;
    unsafe {
        asm!("rdtscp", out("rdx") _, out("rax") _, out("rcx") cpu_id);
    }
    cpu_id
}

/// 5M cpu_id calls in 3s410ms622µs880ns (on my system)
#[inline(always)]
fn _cpu_id_tsc_msr() -> usize {
    let tsc = Msr::new(IA32_TSC_AUX);
    unsafe { tsc.read() as _ }
}

/* fn benchmark() -> ! {
    drivers::lazy_install_early(VFS_ROOT.clone());
    drivers::lazy_install_late();
    let mut i = 0usize;
    let start = hyperion_instant::Instant::now();
    println!("cpuid ty: {}", hyperion_cpu_id::cpu_id_dyn_type());
    for _ in 0..5_000_000 {
        i += core::hint::black_box(cpu_id)();
    }
    core::hint::black_box(i);
    println!("5M cpu_id calls in {}", start.elapsed());
    panic!();
}
benchmark(); */

/// initialize [`cpu_id`]
pub fn init() {
    static CPU_ID_GEN: AtomicUsize = AtomicUsize::new(0);
    let cpu_id = CPU_ID_GEN.fetch_add(1, Ordering::SeqCst);

    select_cpu_id_dyn();

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
