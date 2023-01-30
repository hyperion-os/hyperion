use crate::mem::map::Memmap;
use limine::{LimineMemmapEntry, LimineMemmapRequest, LimineMemoryMapEntryType, NonNullPtr};
use spin::Lazy;

//

pub fn memmap() -> impl Iterator<Item = Memmap> {
    const DEFAULT_MEMMAP: Memmap = Memmap {
        base: u64::MAX,
        len: 0u64,
    };

    memiter()
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

pub fn memtotal() -> u64 {
    static TOTAL: Lazy<u64> = Lazy::new(|| {
        memiter()
            .filter(|memmap| {
                memmap.typ != LimineMemoryMapEntryType::Reserved
                    && memmap.typ != LimineMemoryMapEntryType::Framebuffer
            })
            .map(|memmap| memmap.len)
            .sum::<u64>()
    });
    *TOTAL
}

fn memiter() -> impl Iterator<Item = &'static NonNullPtr<LimineMemmapEntry>> {
    static REQ: LimineMemmapRequest = LimineMemmapRequest::new(0);
    REQ.get_response()
        .get()
        .into_iter()
        .flat_map(|a| a.memmap())
}
