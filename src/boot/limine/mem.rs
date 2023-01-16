use crate::mem::Memmap;
use limine::{LimineMemmapRequest, LimineMemoryMapEntryType};

//

pub fn memmap() -> impl Iterator<Item = Memmap> {
    static REQ: LimineMemmapRequest = LimineMemmapRequest::new(0);

    const DEFAULT_MEMMAP: Memmap = Memmap {
        base: u64::MAX,
        len: 0u64,
    };

    REQ.get_response()
        .get()
        .into_iter()
        .flat_map(|a| a.memmap())
        .scan(DEFAULT_MEMMAP, |acc, memmap| {
            // TODO: zero init reclaimable regions
            if let LimineMemoryMapEntryType::Usable
            // | LimineMemoryMapEntryType::AcpiReclaimable
            // | LimineMemoryMapEntryType::BootloaderReclaimable
            = memmap.typ
            {
                acc.base = memmap.base.min(acc.base);
                acc.len += memmap.len;
                Some(None)
            } else if acc.len == 0 {
                acc.base = u64::MAX;
                Some(None)
            } else {
                Some(Some(core::mem::replace(acc, DEFAULT_MEMMAP)))
            }
        })
        .flatten()
}
