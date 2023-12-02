use alloc::{vec, vec::Vec};
use core::{
    fmt::Debug,
    marker::PhantomData,
    ops::Range,
    sync::atomic::{AtomicU64, Ordering},
};

use crossbeam::queue::SegQueue;
use hyperion_mem::{
    pmm::{self, PageFrame},
    vmm::{Handled, NotHandled, PageFaultResult, PageMapImpl},
};
use x86_64::{structures::paging::PageTableFlags, PhysAddr, VirtAddr};

use crate::vmm::PageMap;

//

/// the first frame of the stack
pub const USER_STACK_TOP: u64 = 0x7FFD_FFFF_F000; // 0x8000_0000_0000;

pub const VIRT_STACK_PAGES: u64 = 512;
pub const VIRT_STACK_SIZE: u64 = 0x1000 * VIRT_STACK_PAGES; // 2MiB (contains the 4KiB guard page)
pub const VIRT_STACK_SIZE_ALL: u64 = VIRT_STACK_SIZE * MAX_STACK_COUNT;

/// also the max thread count per process
pub const MAX_STACK_COUNT: u64 = 0x1000;

pub const USER_HEAP_TOP: u64 = USER_STACK_TOP - VIRT_STACK_SIZE * MAX_STACK_COUNT;

//

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KernelStack;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UserStack;

pub trait StackType {
    fn region() -> Range<u64>;

    const PAGE_FLAGS: PageTableFlags;
    const TY: &'static str;
}

impl StackType for KernelStack {
    fn region() -> Range<u64> {
        let top = hyperion_boot::virt_addr() as u64 - 0x1000;
        let bottom = top - (hyperion_boot::hhdm_offset() + 0x10000000000u64);

        bottom..top

        // USER_HEAP_TOP..USER_STACK_TOP
    }

    const PAGE_FLAGS: PageTableFlags = PageTableFlags::from_bits_truncate(
        PageTableFlags::PRESENT.bits()
            | PageTableFlags::WRITABLE.bits()
            | PageTableFlags::NO_EXECUTE.bits(),
    );

    const TY: &'static str = "kernel";
}

impl StackType for UserStack {
    fn region() -> Range<u64> {
        USER_HEAP_TOP..USER_STACK_TOP
    }

    const PAGE_FLAGS: PageTableFlags = PageTableFlags::from_bits_truncate(
        PageTableFlags::USER_ACCESSIBLE.bits()
            | PageTableFlags::PRESENT.bits()
            | PageTableFlags::WRITABLE.bits()
            | PageTableFlags::NO_EXECUTE.bits(),
    );

    const TY: &'static str = "user";
}

//

pub struct Stacks<StackType> {
    free_stacks: SegQueue<u64>,
    next_stack: AtomicU64,
    limit: u64,

    _p: PhantomData<StackType>,
}

impl<T: StackType + Debug> Stacks<T> {
    pub fn new() -> Self {
        let region = T::region();

        Self {
            free_stacks: SegQueue::new(),
            next_stack: AtomicU64::new(region.end),
            limit: region.start,

            _p: PhantomData,
        }
    }

    /// # Safety
    ///
    /// the stack is not safe to use before initializing it
    pub unsafe fn take(&self) -> Stack<T> {
        let top = self
            .free_stacks
            .pop()
            .unwrap_or_else(|| self.next_stack.fetch_sub(VIRT_STACK_SIZE, Ordering::SeqCst));

        if top <= self.limit {
            todo!("recover from reached stack limit");
        }

        Stack::new(VirtAddr::new(top))
    }

    pub fn take_lazy(&self, page_map: &PageMap) -> Stack<T> {
        // SAFETY: the stack gets initialized
        let stack = unsafe { self.take() };
        stack.init(page_map);
        stack
    }

    pub fn take_prealloc(&self, page_map: &PageMap, size_4k_pages: u64) -> Stack<T> {
        // SAFETY: the stack gets initialized
        let mut stack = unsafe { self.take() };
        if let Some(grow) = size_4k_pages.checked_sub(stack.extent_4k_pages) {
            _ = stack.grow(page_map, grow);
        }
        stack
    }

    pub fn free(&self, stack: Stack<T>) {
        self.free_stacks.push(stack.top.as_u64());
    }
}

impl<T: StackType + Debug> Default for Stacks<T> {
    fn default() -> Self {
        Self::new()
    }
}

//

pub struct AddressSpace {
    pub page_map: PageMap,

    pub user_stacks: Stacks<UserStack>,
    pub kernel_stacks: Stacks<KernelStack>,
}

impl AddressSpace {
    pub fn new(page_map: PageMap) -> Self {
        Self {
            page_map,
            user_stacks: Stacks::new(),
            kernel_stacks: Stacks::new(),
        }
    }

    pub fn take_user_stack_lazy(&self) -> Stack<UserStack> {
        self.user_stacks.take_lazy(&self.page_map)
    }

    pub fn take_user_stack_prealloc(&self, size_4k_pages: u64) -> Stack<UserStack> {
        self.user_stacks
            .take_prealloc(&self.page_map, size_4k_pages)
    }

    pub fn take_kernel_stack_lazy(&self) -> Stack<KernelStack> {
        self.kernel_stacks.take_lazy(&self.page_map)
    }

    pub fn take_kernel_stack_prealloc(&self, size_4k_pages: u64) -> Stack<KernelStack> {
        self.kernel_stacks
            .take_prealloc(&self.page_map, size_4k_pages)
    }
}

//

/// stacks have a guard page to trigger the page fault
///
/// multiple stacks
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Stack<StackType> {
    /// size of the stack in 4k pages,
    /// used in PageFault stack growing
    pub extent_4k_pages: u64,

    /// limit how much the stack is allowed to grow,
    /// in 4k pages again
    pub limit_4k_pages: u64,

    /// the location of where the top of the stack is mapped in virtual memory
    pub top: VirtAddr,

    // TODO: init alloc size, default: 1 page
    pub base_alloc: PhysAddr,
    pub extra_alloc: Vec<PhysAddr>,

    _p: PhantomData<StackType>,
}

#[derive(Debug, Clone, Copy)]
pub struct StackLimitHit;

impl<T: StackType + Debug> Stack<T> {
    pub fn new(top: VirtAddr) -> Self {
        Self::with_limit(top, VIRT_STACK_PAGES)
    }

    pub fn with_limit(top: VirtAddr, mut limit_4k_pages: u64) -> Self {
        limit_4k_pages = limit_4k_pages.min(VIRT_STACK_PAGES);

        Self {
            extent_4k_pages: 0,
            limit_4k_pages,
            top,
            base_alloc: PhysAddr::new(0),
            extra_alloc: vec![],
            _p: PhantomData,
        }
    }

    pub fn guard_page(&self) -> VirtAddr {
        self.top - 0x1000u64 * (self.extent_4k_pages + 1)
    }

    fn page_range(page: VirtAddr) -> Range<VirtAddr> {
        page..page + 0x1000u64
    }

    /// won't allocate the stack,
    /// this only makes sure the guard page is there
    pub fn init(&self, page_map: &PageMap) {
        hyperion_log::trace!("init a stack {:?}", T::PAGE_FLAGS);

        // page_map.activate();
        page_map.unmap(Self::page_range(self.guard_page()));
    }

    pub fn grow(&mut self, page_map: &PageMap, grow_by_pages: u64) -> Result<(), StackLimitHit> {
        hyperion_log::trace!("growing a stack {:?}", T::PAGE_FLAGS);

        if self.extent_4k_pages + grow_by_pages > self.limit_4k_pages {
            hyperion_log::trace!("stack limit hit");
            // stack cannot grow anymore
            return Err(StackLimitHit);
        }

        let old_guard_page = Self::page_range(self.guard_page());

        let first_time = self.extent_4k_pages == 0;
        self.extent_4k_pages += grow_by_pages;
        let new_guard_page = Self::page_range(self.guard_page());

        let alloc = pmm::PFA.alloc(grow_by_pages as usize).physical_addr();

        if first_time {
            // TODO: init alloc size, default: 1 page
            self.base_alloc = alloc;
        } else {
            self.extra_alloc.push(alloc);
        }

        page_map.map(new_guard_page.end..old_guard_page.end, alloc, T::PAGE_FLAGS);
        page_map.unmap(new_guard_page);

        Ok(())
    }

    pub fn page_fault(&mut self, page_map: &PageMap, addr: u64) -> PageFaultResult {
        let addr = VirtAddr::new(addr);

        // just making sure the correct page_map was mapped
        // TODO: assert
        // page_map.activate();
        assert!(page_map.is_active());

        hyperion_log::trace!("stack page fault test ({})", T::TY);

        let guard_page = self.guard_page();

        if !(guard_page..=guard_page + 0xFFFu64).contains(&addr) {
            hyperion_log::trace!("guard page not hit (0x{guard_page:016x})");
            // guard page not hit, so its not a stack overflow
            return Ok(NotHandled);
        }

        // TODO: configurable grow_by_rate
        if let Err(StackLimitHit) = self.grow(page_map, 1) {
            return Ok(NotHandled);
        }

        hyperion_log::trace!(
            "now {addr:018x} maps to {:018x?}",
            page_map.virt_to_phys(addr)
        );

        Err(Handled)
    }

    pub fn cleanup(&mut self, page_map: &PageMap) {
        if self.extent_4k_pages == 0 {
            return;
        }

        page_map.unmap(self.top - self.extent_4k_pages * 0x1000..self.top);

        for alloc in self.extra_alloc.drain(..).chain([self.base_alloc]) {
            let base_alloc = unsafe { PageFrame::new(alloc, 1) };
            pmm::PFA.free(base_alloc);
        }
    }
}

/* use alloc::vec::Vec;

use hyperion_arch::vmm::PageMap;
use hyperion_mem::{
    pmm::{PageFrame, PageFrameAllocator},
    vmm::PageMapImpl,
};
use x86_64::PhysAddr;

//

pub struct Stack {
    frame: Option<PhysAddr>,
    frames: Vec<PageFrame>,
}

//

impl Stack {
    pub fn new(a_spc: &PageMap) -> Self {
        a_spc.unmap(v_addr);
    }

    pub fn page_fault_handler(&self, addr: usize) {}
}

impl Drop for Stack {
    fn drop(&mut self) {
        if let Some(frames) = self.frames.take() {
            PageFrameAllocator::get().free(stack)
        }
    }
} */
