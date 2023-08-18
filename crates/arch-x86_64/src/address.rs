use alloc::{vec, vec::Vec};
use core::{fmt::Debug, marker::PhantomData, ops::Range, sync::atomic::AtomicU64};

use crossbeam::atomic::AtomicCell;
use hyperion_mem::{
    pmm::{PageFrame, PageFrameAllocator},
    vmm::PageMapImpl,
};
use spin::RwLock;
use x86_64::{
    structures::{idt::PageFaultErrorCode, paging::PageTableFlags},
    PhysAddr, VirtAddr,
};

use crate::vmm::PageMap;

//

/// the first frame of the stack
pub const KERNEL_STACK_BASE: u64 = 0xFFFF_FFFF_FFFF_F000;
pub const KERNEL_RSP: u64 = 0x0; // stackyeet
/// the first frame of the stack
pub const USER_STACK_BASE: u64 = 0x7FFF_FFFF_F000;
pub const USER_RSP: u64 = 0x8000_0000_0000;

//

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PageFaultResult {
    Handled,
    NotHandled,
}

//

impl PageFaultResult {
    pub const fn is_handled(self) -> bool {
        matches!(self, PageFaultResult::Handled)
    }

    pub const fn is_not_handled(self) -> bool {
        matches!(self, PageFaultResult::NotHandled)
    }
}

//

pub struct AddressSpace {
    pub page_map: PageMap,

    pub kernel_stack: RwLock<Stack<KernelStack>>,
    pub user_stack: RwLock<Stack<UserStack>>,
}

impl AddressSpace {
    pub fn new(page_map: PageMap) -> Self {
        let kernel_stack = Stack::new();
        kernel_stack.init(&page_map);

        let user_stack = Stack::new();
        user_stack.init(&page_map);

        Self {
            page_map,
            kernel_stack: RwLock::new(kernel_stack),
            user_stack: RwLock::new(user_stack),
        }
    }

    pub fn init_address_spaces(&self, ip: VirtAddr) -> ! {
        hyperion_log::debug!("jumping to 0x{ip:016x}");

        unsafe {
            core::arch::asm!(
                "mov rsp, {stack}",
                "jmp {ip}",
                ip = in(reg) ip.as_u64(),
                stack = const(KERNEL_RSP)
            )
        };

        unreachable!();
    }

    pub fn switch_to(&self) {
        // TODO: assert
        // in kernel code, rsp should always be `KERNEL_STACK_BASE`
        // and in user code, rsp should always be `USER_STACK_BASE`

        self.page_map.activate();

        /* unsafe {
            core::arch::asm!(
                "mov rsp, {stack}",
                "",
                stack = const(KERNEL_RSP)
            );

            // TODO: fx(save/rstor)64 (rd/wr)(fs/gs)base

            unsafe {
                core::arch::asm!(
                    // save callee-saved registers
                    // https://wiki.osdev.org/System_V_ABI
                    "push rbp",
                    "push rbx",
                    "push r12",
                    "push r13",
                    "push r14",
                    "push r15",

                    // save prev task
                    "mov [rdi+{rsp}], rsp", // save prev stack

                    // load next task
                    "mov rsp, [rsi+{rsp}]", // load next stack
                    "mov rax, [rsi+{cr3}]", // rax = next virtual address space
                    // TODO: load TSS privilege stack

                    // optional virtual address space switch
                    "mov rcx, cr3", // rcx = prev virtual address space
                    "cmp rax, rcx", // cmp for if
                    "je 2f",         // if rax != rcx:
                    "mov cr3, rax", // load next virtual address space

                    "2:",

                    "pop r15",
                    "pop r14",
                    "pop r13",
                    "pop r12",
                    "pop rbx",
                    "pop rbp",

                    "ret",

                    rsp = const(offset_of!(Context, rsp)),
                    cr3 = const(offset_of!(Context, cr3)),
                    options(noreturn)
                );
            }
        } */
    }

    pub fn page_fault(&self, addr: VirtAddr, ec: PageFaultErrorCode) -> PageFaultResult {
        if ec.contains(PageFaultErrorCode::USER_MODE) {
            self.user_stack.write().page_fault(&self.page_map, addr)
        } else {
            self.kernel_stack.write().page_fault(&self.page_map, addr)
        }
    }
}

//

/// stacks are lazy allocated with a page fault
///
/// stacks have a guard page to trigger the page fault
///
/// kernel stacks are at `..0xFFFF_FFFF_FFFF_FFFF` ([`KERNEL_STACK_BASE`])
///   user stacks are at `..0x0000_7FFF_FFFF_FFFF` ([`USER_STACK_BASE`])
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

    // TODO: init alloc size, default: 1 page
    pub base_alloc: PhysAddr,
    pub extra_alloc: Vec<PhysAddr>,

    _p: PhantomData<StackType>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KernelStack;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UserStack;

pub trait StackType {
    const BASE: u64;

    const PAGE_FLAGS: PageTableFlags;
}

impl<T: StackType + Debug> Stack<T> {
    pub const fn new() -> Self {
        Self::with_limit(16)
    }

    pub const fn with_limit(limit_4k_pages: u64) -> Self {
        Self {
            extent_4k_pages: 0,
            limit_4k_pages,
            base_alloc: PhysAddr::new(0),
            extra_alloc: vec![],
            _p: PhantomData,
        }
    }

    pub const fn guard_page(&self) -> u64 {
        T::BASE - 0x1000u64 * self.extent_4k_pages
    }

    fn page_range(page: u64) -> Range<VirtAddr> {
        VirtAddr::new(page)..VirtAddr::new(page.saturating_add(0x1000u64))
    }

    /// won't allocate the stack,
    /// this only makes sure the guard page is there
    pub fn init(&self, page_map: &PageMap) {
        page_map.activate();
        page_map.unmap(Self::page_range(self.guard_page()));
    }

    pub fn page_fault(&mut self, page_map: &PageMap, addr: VirtAddr) -> PageFaultResult {
        // just making sure the correct page_map was mapped
        // TODO: assert
        page_map.activate();

        hyperion_log::trace!("stack page fault test\n{self:#?}");

        let guard_page = self.guard_page();

        if !(guard_page..=guard_page + 0xFFF).contains(&addr.as_u64()) {
            hyperion_log::trace!("guard page not hit (0x{guard_page:016x})");
            // guard page not hit, so its not a stack overflow
            return PageFaultResult::NotHandled;
        }

        if self.extent_4k_pages == self.limit_4k_pages {
            hyperion_log::trace!("stack limit hit");
            // stack cannot grow anymore
            return PageFaultResult::NotHandled;
        }

        let old_guard_page = guard_page;

        let first_time = self.extent_4k_pages == 0;
        self.extent_4k_pages += 1;
        let new_guard_page = self.guard_page();

        let alloc = PageFrameAllocator::get().alloc(1).physical_addr();

        if first_time {
            // TODO: init alloc size, default: 1 page
            self.base_alloc = alloc;
        } else {
            self.extra_alloc.push(alloc);
        }

        page_map.map(
            Self::page_range(old_guard_page),
            self.base_alloc,
            T::PAGE_FLAGS,
        );
        page_map.unmap(Self::page_range(new_guard_page));

        PageFaultResult::Handled
    }
}

impl<T> Drop for Stack<T> {
    fn drop(&mut self) {
        if self.extent_4k_pages == 0 {
            return;
        }

        for alloc in self.extra_alloc.drain(..).chain([self.base_alloc]) {
            let base_alloc = unsafe { PageFrame::new(alloc, 1) };
            PageFrameAllocator::get().free(base_alloc);
        }
    }
}

impl StackType for KernelStack {
    const BASE: u64 = KERNEL_STACK_BASE;

    const PAGE_FLAGS: PageTableFlags = PageTableFlags::from_bits_truncate(
        PageTableFlags::PRESENT.bits() | PageTableFlags::WRITABLE.bits(),
    );
}

impl StackType for UserStack {
    const BASE: u64 = USER_STACK_BASE;

    const PAGE_FLAGS: PageTableFlags = PageTableFlags::from_bits_truncate(
        PageTableFlags::USER_ACCESSIBLE.bits()
            | PageTableFlags::PRESENT.bits()
            | PageTableFlags::WRITABLE.bits(),
    );
}
