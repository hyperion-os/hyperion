use memoffset::offset_of;
use x86_64::{registers::control::Cr3, PhysAddr, VirtAddr};

//

#[derive(Debug)]
#[repr(C)]
pub struct Context {
    pub rsp: VirtAddr,
    pub cr3: PhysAddr,
}

impl Context {
    pub fn new(stack: &mut [u64], thread_entry: extern "sysv64" fn() -> !) -> Self {
        let [top @ .., _r15, _r14, _r13, _r12, _rbx, _rbp, entry] = stack else {
            unreachable!("the stack is too small")
        };

        *entry = thread_entry as *const () as u64;

        Self {
            cr3: Cr3::read().0.start_address(),
            rsp: VirtAddr::new(top.as_ptr_range().end as u64),
        }
    }
}

//

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
