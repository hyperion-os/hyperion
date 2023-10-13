use core::{arch::asm, mem::size_of, sync::atomic::Ordering};

use hyperion_log::debug;
use hyperion_mem::{to_higher_half, vmm::PageMapImpl};
use memoffset::offset_of;
use x86_64::{registers::model_specific::KernelGsBase, PhysAddr, VirtAddr};

use crate::{dbg_cpu, tls::ThreadLocalStorage, vmm::PageMap};

//

#[derive(Debug)]
#[repr(C)]
pub struct Context {
    pub rsp: VirtAddr,
    pub cr3: PhysAddr,
    pub syscall_stack: VirtAddr,
}

impl Context {
    pub fn new(
        page_map: &PageMap,
        stack_top: VirtAddr, // TODO: could be a &mut [u64],
        thread_entry: extern "sysv64" fn() -> !,
    ) -> Self {
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
        let now = page_map
            .virt_to_phys(rsp)
            .expect("stack to be mapped in the new page table");
        let now = to_higher_half(now);

        let init: *mut StackInit = now.as_mut_ptr();
        unsafe {
            init.write(StackInit {
                _r15: 0,
                _r14: 0,
                _r13: 0,
                _r12: 0,
                _rbx: 0,
                _rbp: 0,
                entry: thread_entry as *const () as _,
            });
        }

        Self {
            cr3: page_map.cr3().start_address(),
            rsp,
            syscall_stack: stack_top,
        }
    }

    /// # Safety
    ///
    /// this task is not safe to switch to
    pub unsafe fn invalid(page_map: &PageMap) -> Self {
        Self {
            cr3: page_map.cr3().start_address(),
            rsp: VirtAddr::new_truncate(0),
            syscall_stack: VirtAddr::new_truncate(0),
        }
    }
}

//

/// # Safety
///
/// both `prev` and `next` must be correct and point to valid exclusive [`Context`] values
/// even after switching the new address spacing according to the field `cr3` in `next`
pub unsafe fn switch(prev: *mut Context, next: *mut Context) {
    let tls: &'static ThreadLocalStorage = unsafe { &*KernelGsBase::read().as_ptr() };
    let next_syscall_stack = unsafe { (*next).syscall_stack.as_mut_ptr() };

    tls.kernel_stack.store(next_syscall_stack, Ordering::SeqCst);

    // debug!("ctx switch, new gs:kernel_stack={next_syscall_stack:018x?}");
    // dbg_cpu();

    unsafe { switch_inner(prev, next) };

    // dbg_cpu();
}

#[naked]
unsafe extern "sysv64" fn switch_inner(prev: *mut Context, next: *mut Context) {
    // TODO: fx(save/rstor)64 (rd/wr)(fs/gs)base
    unsafe {
        asm!(
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
}
