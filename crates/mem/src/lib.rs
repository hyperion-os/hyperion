#![no_std]
#![feature(int_roundings, pointer_is_aligned, allocator_api, maybe_uninit_slice)]

//

extern crate alloc;

use hyperion_boot::hhdm_offset;
use hyperion_slab_alloc::{PageFrameAllocator, PageFrames, SlabAllocator};
use x86_64::{PhysAddr, VirtAddr};

use crate::pmm::{PageFrame, PFA};

//

pub mod pmm;
pub mod vmm;

//

pub type KernelSlabAlloc<Lock> = SlabAllocator<Pfa, Lock>;

//

pub struct Pfa;

impl PageFrameAllocator for Pfa {
    fn alloc(pages: usize) -> PageFrames {
        let pages = PFA.alloc(pages);

        unsafe { PageFrames::new(pages.virtual_addr().as_mut_ptr(), pages.len()) }
    }

    fn free(frames: PageFrames) {
        let pages = unsafe {
            PageFrame::new(
                from_higher_half(VirtAddr::new(frames.as_ptr() as u64)),
                frames.len(),
            )
        };

        PFA.free(pages);
    }
}

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
    addr >= hhdm_offset()
}

#[allow(unused)]
pub fn to_higher_half(addr: PhysAddr) -> VirtAddr {
    let addr = addr.as_u64();
    if is_higher_half(addr) {
        VirtAddr::new(addr)
    } else {
        VirtAddr::new(addr + hhdm_offset())
    }
}

#[allow(unused)]
pub fn from_higher_half(addr: VirtAddr) -> PhysAddr {
    let addr = addr.as_u64();
    if is_higher_half(addr) {
        PhysAddr::new(addr - hhdm_offset())
    } else {
        PhysAddr::new(addr)
    }
}
