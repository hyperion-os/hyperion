use core::{
    arch::asm,
    ops::Range,
    sync::atomic::{AtomicBool, AtomicUsize, Ordering},
};

use hyperion_boot_interface::{Memmap, Memtype};
use hyperion_log::trace;
use limine::{LimineMemmapEntry, LimineMemmapRequest, LimineMemoryMapEntryType, NonNullPtr};

//

pub fn memmap() -> impl Iterator<Item = Memmap> {
    static FIRST_TIME: AtomicBool = AtomicBool::new(true);
    let first_time = FIRST_TIME.swap(false, Ordering::SeqCst);

    memiter().filter_map(move |memmap| {
        // TODO: zero init reclaimable regions

        if first_time {
            trace!(
                "[ {:#018x?} ]: {:?}",
                memmap.base..memmap.base + memmap.len,
                memmap.typ
            );
        }

        let ty = match memmap.typ {
            LimineMemoryMapEntryType::Usable => Memtype::Usable,
            LimineMemoryMapEntryType::BootloaderReclaimable => Memtype::BootloaderReclaimable,
            LimineMemoryMapEntryType::KernelAndModules => Memtype::KernelAndModules,
            LimineMemoryMapEntryType::Framebuffer => Memtype::Framebuffer,
            _ => return None,
        };

        Some(Memmap {
            base: memmap.base as _,
            len: memmap.len as _,
            ty,
        })
    })
}

pub fn stack() -> Range<usize> {
    let top = STACK_TOP.load(Ordering::SeqCst);
    top - 0x10000..top
}

#[inline(always)]
pub fn stack_init() {
    let stack_ptr: usize;
    unsafe {
        asm!("mov {}, rsp", out(reg) stack_ptr);
    }
    STACK_TOP.store(stack_ptr, Ordering::SeqCst);
}

fn memiter() -> impl Iterator<Item = &'static NonNullPtr<LimineMemmapEntry>> {
    static REQ: LimineMemmapRequest = LimineMemmapRequest::new(0);
    REQ.get_response()
        .get()
        .into_iter()
        .flat_map(|a| a.memmap())
}

//

static STACK_TOP: AtomicUsize = AtomicUsize::new(0);
