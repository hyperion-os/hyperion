use crate::{boot, debug};
use x86_64::{
    registers::control::Cr3,
    structures::paging::{page_table::FrameError, PageTable, PhysFrame, Size2MiB, Size4KiB},
    PhysAddr, VirtAddr,
};

//

pub mod map;

// allocator
pub mod bump;
pub mod pmm;

//

#[allow(unused)]
fn is_higher_half(addr: u64) -> bool {
    addr >= boot::hhdm_offset()
}

#[allow(unused)]
fn to_higher_half(addr: PhysAddr) -> VirtAddr {
    let addr = addr.as_u64();
    if is_higher_half(addr) {
        VirtAddr::new(addr)
    } else {
        VirtAddr::new(addr + boot::hhdm_offset())
    }
}

#[allow(unused)]
fn from_higher_half(addr: VirtAddr) -> PhysAddr {
    let addr = addr.as_u64();
    if is_higher_half(addr) {
        PhysAddr::new(addr - boot::hhdm_offset())
    } else {
        PhysAddr::new(addr)
    }
}

fn walk_page_tables(addr: VirtAddr) -> Option<PhysAddr> {
    enum AnyPhysFrame {
        Size4KiB(PhysFrame<Size4KiB>),
        Size2MiB(PhysFrame<Size2MiB>),
    }

    impl AnyPhysFrame {
        fn start_address(&self) -> PhysAddr {
            match self {
                AnyPhysFrame::Size4KiB(v) => v.start_address(),
                AnyPhysFrame::Size2MiB(v) => v.start_address(),
            }
        }
    }

    let (l4, _) = Cr3::read();

    let page_table_indices = [
        addr.p4_index(),
        addr.p3_index(),
        addr.p2_index(),
        addr.p1_index(),
    ];
    let mut frame = AnyPhysFrame::Size4KiB(l4);

    for index in page_table_indices {
        let virt = to_higher_half(frame.start_address());
        let table: *const PageTable = virt.as_ptr();
        let table = unsafe { &*table };

        let entry = &table[index];
        frame = match entry.frame() {
            Ok(frame) => AnyPhysFrame::Size4KiB(frame),
            Err(FrameError::FrameNotPresent) => return None,
            Err(FrameError::HugeFrame) => {
                AnyPhysFrame::Size2MiB(PhysFrame::<Size2MiB>::containing_address(entry.addr()))
            }
        }
    }

    Some(frame.start_address() + u64::from(addr.page_offset()))
}

#[allow(unused)]
fn debug_phys_addr(addr: PhysAddr) {
    debug!(
        "{:?} {:?} {:?}",
        addr,
        walk_page_tables(VirtAddr::new(addr.as_u64())),
        walk_page_tables(to_higher_half(addr))
    );
}
