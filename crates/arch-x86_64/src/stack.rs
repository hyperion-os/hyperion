use core::{
    fmt::Debug,
    marker::PhantomData,
    ops::Range,
    sync::atomic::{AtomicU64, Ordering},
};

use crossbeam::queue::SegQueue;
use hyperion_log::*;
use hyperion_mem::{
    pmm,
    vmm::{MapTarget, PageMapImpl},
};
use x86_64::{structures::paging::PageTableFlags, VirtAddr};

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
        hyperion_log::info!("{:x}", VirtAddr::new_truncate(510 << 39));
        let top = hyperion_boot::virt_addr() as u64 - 0x1000;
        let bottom = top - 0x10000000000u64;

        bottom..top

        // USER_HEAP_TOP..USER_STACK_TOP
    }

    const PAGE_FLAGS: PageTableFlags = PageTableFlags::from_bits_truncate(
        PageTableFlags::WRITABLE.bits() | PageTableFlags::NO_EXECUTE.bits(),
    );

    const TY: &'static str = "kernel";
}

impl StackType for UserStack {
    fn region() -> Range<u64> {
        USER_HEAP_TOP..USER_STACK_TOP
    }

    const PAGE_FLAGS: PageTableFlags = PageTableFlags::from_bits_truncate(
        PageTableFlags::USER_ACCESSIBLE.bits()
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
    pub unsafe fn take_no_init(&self) -> Stack<T> {
        let top = self
            .free_stacks
            .pop()
            .unwrap_or_else(|| self.next_stack.fetch_sub(VIRT_STACK_SIZE, Ordering::SeqCst));

        if top <= self.limit {
            todo!("recover from reached stack limit");
        }

        Stack::new(VirtAddr::new(top))
    }

    pub fn take(&self, page_map: &PageMap) -> Stack<T> {
        // SAFETY: the stack gets initialized
        let stack = unsafe { self.take_no_init() };
        stack.init(page_map);
        stack
    }

    pub fn take_force_init(&self, page_map: &PageMap, forced_pages: u64) -> Stack<T> {
        // SAFETY: the stack gets initialized
        let stack = unsafe { self.take_no_init() };
        stack.force_init(page_map, forced_pages);
        stack
    }

    pub fn free(&self, page_map: &PageMap, stack: Stack<T>) {
        self.free_stacks.push(stack.top.as_u64());
        stack.dealloc(page_map);
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

    pub fn fork(&self, keep_user: &Stack<UserStack>) -> Self {
        let page_map = self.page_map.fork();

        let user_stacks = Stacks::new();
        loop {
            // TODO: improve this
            // find and lock the correct stack
            let try_stack = unsafe { user_stacks.take_no_init() };
            if try_stack.top == keep_user.top {
                break;
            }
            user_stacks.free_stacks.push(try_stack.top.as_u64());
        }

        Self {
            page_map,
            user_stacks,
            kernel_stacks: Stacks::new(),
        }
    }

    pub fn take_user_stack(&self) -> Stack<UserStack> {
        self.user_stacks.take(&self.page_map)
    }

    pub fn take_user_stack_prealloc(&self, forced_pages: u64) -> Stack<UserStack> {
        self.user_stacks
            .take_force_init(&self.page_map, forced_pages)
    }

    pub fn take_kernel_stack(&self) -> Stack<KernelStack> {
        self.kernel_stacks.take(&self.page_map)
    }

    pub fn take_kernel_stack_prealloc(&self, forced_pages: u64) -> Stack<KernelStack> {
        self.kernel_stacks
            .take_force_init(&self.page_map, forced_pages)
    }
}

//

/// stacks have a guard page to trigger the page fault
///
/// multiple stacks
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Stack<StackType> {
    /// limit how much the stack is allowed to grow,
    /// in 4k pages again
    pub limit_4k_pages: u64,

    /// the location of where the top of the stack is mapped in virtual memory
    pub top: VirtAddr,

    _p: PhantomData<StackType>,
}

impl<T> Stack<T> {
    pub const fn empty() -> Self {
        Self {
            limit_4k_pages: 0,
            top: VirtAddr::new_truncate(0),
            _p: PhantomData,
        }
    }
}

impl<T> Default for Stack<T> {
    fn default() -> Self {
        Self::empty()
    }
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
            limit_4k_pages,
            top,
            _p: PhantomData,
        }
    }

    pub fn dealloc(self, page_map: &PageMap) {
        let stack_top = self.top;
        let stack_bottom = stack_top - 0x1000u64 * self.limit_4k_pages;
        page_map.unmap(stack_bottom..stack_top);
    }

    pub fn init(&self, page_map: &PageMap) {
        trace!("init a stack {:?}", T::PAGE_FLAGS);

        let stack_top = self.top;
        let stack_bottom = stack_top - 0x1000u64 * self.limit_4k_pages;
        let guard_top = stack_bottom;
        let guard_bottom = guard_top - 0x1000u64;

        // the VMM allocates lazily
        page_map.map(stack_bottom..stack_top, MapTarget::LazyAlloc, T::PAGE_FLAGS);
        page_map.unmap(guard_bottom..guard_top);
        // page_map.map(guard_bottom..guard_top, None, NO_MAP);
    }

    pub fn force_init(&self, page_map: &PageMap, forced_pages: u64) {
        trace!("init a stack {:?}", T::PAGE_FLAGS);

        let alloc_top = self.top;
        let alloc_bottom = self.top - 0x1000u64 * forced_pages;
        let stack_top = alloc_bottom;
        let stack_bottom =
            alloc_top - 0x1000u64 * self.limit_4k_pages.checked_sub(forced_pages).unwrap();
        let guard_top = stack_bottom;
        let guard_bottom = guard_top - 0x1000u64;

        let alloc = pmm::PFA.alloc(forced_pages as usize);

        page_map.map(
            alloc_bottom..alloc_top,
            MapTarget::Preallocated(alloc.physical_addr()),
            T::PAGE_FLAGS,
        );
        // the VMM allocates lazily
        page_map.map(stack_bottom..stack_top, MapTarget::LazyAlloc, T::PAGE_FLAGS);
        page_map.unmap(guard_bottom..guard_top);
        // page_map.map(guard_bottom..guard_top, None, NO_MAP);
    }
}
