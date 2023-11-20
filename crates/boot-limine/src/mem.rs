use core::sync::atomic::{AtomicBool, Ordering};

use hyperion_boot_interface::{Memmap, Memtype};
use hyperion_log::trace;
use limine::{MemmapEntry, MemmapRequest, MemoryMapEntryType, NonNullPtr};

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
            MemoryMapEntryType::Usable => Memtype::Usable,
            MemoryMapEntryType::BootloaderReclaimable => Memtype::BootloaderReclaimable,
            MemoryMapEntryType::KernelAndModules => Memtype::KernelAndModules,
            MemoryMapEntryType::Framebuffer => Memtype::Framebuffer,
            _ => return None,
        };

        Some(Memmap {
            base: memmap.base as _,
            len: memmap.len as _,
            ty,
        })
    })
}

fn memiter() -> impl Iterator<Item = &'static NonNullPtr<MemmapEntry>> {
    static REQ: MemmapRequest = MemmapRequest::new(0);
    REQ.get_response()
        .get()
        .into_iter()
        .flat_map(|a| a.memmap())
}
