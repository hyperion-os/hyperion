use core::sync::atomic::{AtomicBool, Ordering};

use limine::{LimineMemmapEntry, LimineMemmapRequest, LimineMemoryMapEntryType, NonNullPtr};
use x86_64::PhysAddr;

use crate::{
    mem::map::{Memmap, Memtype},
    trace,
};

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
            _ => return None,
        };

        Some(Memmap {
            base: PhysAddr::new(memmap.base),
            len: memmap.len,
            ty,
        })
    })
}

fn memiter() -> impl Iterator<Item = &'static NonNullPtr<LimineMemmapEntry>> {
    static REQ: LimineMemmapRequest = LimineMemmapRequest::new(0);
    REQ.get_response()
        .get()
        .into_iter()
        .flat_map(|a| a.memmap())
}
