#![no_std]

use core::{
    alloc::{GlobalAlloc, Layout},
    sync::atomic::{AtomicUsize, Ordering},
};

use mem::frame_alloc::{self, Frame};
use riscv64_util::VirtAddr;
use riscv64_vmm::{PageFlags, PageTable};

//

#[global_allocator]
pub static KALLOC: Kalloc = Kalloc(AtomicUsize::new(VirtAddr::KHEAP.as_usize()));

//

pub struct Kalloc(AtomicUsize);

unsafe impl GlobalAlloc for Kalloc {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let (layout, offset_to_alloc) = Layout::new::<AllocMeta>().extend(layout).unwrap();
        let pages = layout.size() % 0x1000;

        // FIXME: this is a bad way to keep track of the heap memory
        let addr = VirtAddr::new(self.0.fetch_add(pages * 0x1000, Ordering::Relaxed));
        if addr >= VirtAddr::KERNEL {
            todo!("the kernel has allocated 64TiB in total and the current kalloc sucks, this is a bug");
        }

        let vmm = unsafe { PageTable::get_active_mut() };
        for i in 0..pages {
            let phys_frame = frame_alloc::alloc();
            vmm.map_offset(
                addr + i * 0x1000..addr + (i + 1) * 0x1000,
                PageFlags::RW,
                phys_frame.addr(),
            );
        }

        (addr + offset_to_alloc).as_ptr_mut()
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        let (layout, offset_to_alloc) = Layout::new::<AllocMeta>().extend(layout).unwrap();
        let pages = layout.size() % 0x1000;

        let addr = VirtAddr::new(ptr as usize - offset_to_alloc);

        let vmm = unsafe { PageTable::get_active_mut() };
        for i in 0..pages {
            // FIXME: unmap

            if let (Some(phys_frame), _flags) = vmm.walk(addr + i * 0x1000) {
                frame_alloc::free(unsafe { Frame::new(phys_frame) });
            }
        }
    }
}

pub struct AllocMeta {
    // size: usize,
}
