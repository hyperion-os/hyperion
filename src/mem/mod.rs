use crate::{boot, debug};
use x86_64::{
    registers::control::Cr3,
    structures::paging::{page_table::FrameError, OffsetPageTable, PageTable, Translate},
    PhysAddr, VirtAddr,
};

//

pub mod map;

// allocator
pub mod bump;
pub mod pmm;

//

#[macro_export]
#[allow(unused)]
macro_rules! debug_phys_addr {
    ($addr:expr) => {
        $crate::debug!(
            "{:?} {:?} {:?}",
            $addr,
            $crate::mem::walk_page_tables(x86_64::VirtAddr::new($addr.as_u64())),
            $crate::mem::walk_page_tables($crate::mem::to_higher_half($addr))
        );
    };
}

#[allow(unused)]
pub fn is_higher_half(addr: u64) -> bool {
    addr >= boot::hhdm_offset()
}

#[allow(unused)]
pub fn to_higher_half(addr: PhysAddr) -> VirtAddr {
    let addr = addr.as_u64();
    if is_higher_half(addr) {
        VirtAddr::new(addr)
    } else {
        VirtAddr::new(addr + boot::hhdm_offset())
    }
}

#[allow(unused)]
pub fn from_higher_half(addr: VirtAddr) -> PhysAddr {
    let addr = addr.as_u64();
    if is_higher_half(addr) {
        PhysAddr::new(addr - boot::hhdm_offset())
    } else {
        PhysAddr::new(addr)
    }
}

pub fn walk_page_tables(addr: VirtAddr) -> Option<PhysAddr> {
    let (l4, _) = Cr3::read();

    let virt = to_higher_half(l4.start_address());
    let table: *mut PageTable = virt.as_mut_ptr();
    let table = unsafe { &mut *table };

    let offs = unsafe { OffsetPageTable::new(table, VirtAddr::new(boot::hhdm_offset())) };

    return offs.translate_addr(addr);

    let page_table_indices = [
        addr.p4_index(),
        addr.p3_index(),
        addr.p2_index(),
        addr.p1_index(),
    ];
    debug!("{page_table_indices:?}");

    page_table_indices
        .into_iter()
        .fold(Some(l4), |acc, index| {
            let frame = acc?;

            let virt = to_higher_half(frame.start_address());
            let table: *const PageTable = virt.as_ptr();
            let table = unsafe { &*table };

            let entry = &table[index];

            match entry.frame() {
                Ok(frame) => Some(frame),
                Err(FrameError::FrameNotPresent) => None,
                Err(FrameError::HugeFrame) => {
                    todo!("Huge pages")
                }
            }
        })
        .map(|frame| frame.start_address() + u64::from(addr.page_offset()))
}
