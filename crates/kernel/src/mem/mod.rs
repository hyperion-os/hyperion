use core::alloc::{GlobalAlloc, Layout};

use hyperion_boot::hhdm_offset;
use spin::Lazy;
use x86_64::{PhysAddr, VirtAddr};

use self::slab::SlabAllocator;


//

pub mod pmm;
pub mod vmm;

// allocator
//pub mod bump;
pub mod slab;

//

pub fn force_init_allocator() {
    Lazy::force(&ALLOCATOR.slab);
}

//

struct KAlloc {
    slab: Lazy<SlabAllocator>,
}

#[global_allocator]
static ALLOCATOR: KAlloc = KAlloc {
    slab: Lazy::new(SlabAllocator::new),
};

//

unsafe impl GlobalAlloc for KAlloc {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        self.slab.alloc(layout.size()).as_mut_ptr()
    }

    unsafe fn dealloc(&self, ptr: *mut u8, _layout: Layout) {
        self.slab.free(VirtAddr::new(ptr as u64))
    }
}

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
