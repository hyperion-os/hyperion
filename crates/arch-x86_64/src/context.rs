use core::mem::size_of;

use hyperion_mem::{to_higher_half, vmm::PageMapImpl};
use memoffset::offset_of;
use x86_64::{registers::control::Cr3, PhysAddr, VirtAddr};

use crate::vmm::PageMap;

//

#[derive(Debug)]
#[repr(C)]
pub struct Context {
    pub rsp: VirtAddr,
    pub cr3: PhysAddr,
}

impl Context {
    pub fn new(
        page_map: &PageMap,
        stack_top: VirtAddr, // &mut [u64],
        thread_entry: extern "sysv64" fn() -> !,
    ) -> Self {
        let cur = PageMap::current();
        page_map.activate();

        let mut res = Self {
            cr3: page_map.cr3().start_address(),
            rsp: stack_top,
        };
        unsafe {
            init(&mut res, thread_entry as usize as u64);
        }

        cur.activate();
        res

        /* hyperion_log::debug!("{stack_top:0x?} {stack_top_now:0x?}");

        #[repr(C)]
        struct StackInit {
            _r15: u64,
            _r14: u64,
            _r13: u64,
            _r12: u64,
            _rbx: u64,
            _rbp: u64,
            entry: u64,
        }

        const OFFSET: usize = size_of::<StackInit>() + size_of::<u64>();

        let rsp = stack_top - OFFSET;
        let now = to_higher_half(stack_top_now - OFFSET);
        hyperion_log::debug!(
            "rsp:{:0x?} now:{:0x?}",
            page_map.virt_to_phys(rsp),
            PageMap::current().virt_to_phys(now)
        );
        let init: *mut StackInit = now.as_mut_ptr();
        unsafe {
            init.write(StackInit {
                _r15: 1,
                _r14: 2,
                _r13: 3,
                _r12: 9,
                _rbx: 5,
                _rbp: 6,
                entry: thread_entry as *const () as _,
            });
        } */

        /* Self {
            cr3: page_map.cr3().start_address(),
            rsp,
        } */
    }
}

//

#[naked]
pub unsafe extern "sysv64" fn init(prev: *mut Context, ra: u64) {
    core::arch::asm!(
        "mov r11, rsp",
        "mov rsp, [rdi+{rsp}]",
        "push rsi",
        "push rbp",
        "push rbx",
        "push r12",
        "push r13",
        "push r14",
        "push r15",
        "mov [rdi+{rsp}], rsp",
        "mov rsp, r11",
        "ret",
        rsp = const(offset_of!(Context, rsp)),
        options(noreturn),
    );
}

/* #[naked]
pub unsafe extern "sysv64" fn enter(next: *mut Context) {
} */

/// # Safety
///
/// both `prev` and `next` must be correct and point to valid exclusive [`Context`] values
/// even after switching the new address spacing according to the field `cr3` in `next`
#[naked]
pub unsafe extern "sysv64" fn switch(prev: *mut Context, next: *mut Context) {
    // TODO: fx(save/rstor)64 (rd/wr)(fs/gs)base

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
        // "push rdi",
        // "push rsi",
        // "call {debug}",
        // "pop rsi",
        // "pop rdi",
        // "push rdi",
        // "push rsi",
        // "mov rdi, [rdi+{rsp}]",
        // "mov rsi, [rsi+{rsp}]",
        // "call {debug}",
        // "pop rsi",
        // "pop rdi",

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
        // debug = sym debug,
        options(noreturn)
    );
}

extern "sysv64" fn debug(rdi: u64, rsi: u64) {
    hyperion_log::debug!("context switch debug: RDI:{rdi:#0x} RSI:{rsi:#0x}");
}
