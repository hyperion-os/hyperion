use core::{arch::naked_asm, fmt, mem::offset_of, sync::atomic::Ordering};

use hyperion_mem::pmm;
use x86_64::{
    registers::{
        model_specific::{Efer, EferFlags, KernelGsBase, LStar, SFMask, Star},
        mxcsr::{self, MxCsr},
        rflags::RFlags,
    },
    VirtAddr,
};

use crate::{cpu::gdt::SegmentSelectors, tls::ThreadLocalStorage};

//

/// init `syscall` and `sysret`
pub fn init(selectors: SegmentSelectors, handler: SyscallHandler) {
    let tls: &'static ThreadLocalStorage = unsafe { &*KernelGsBase::read().as_ptr() };
    let kernel_syscall_stack = pmm::PFA.alloc(8).leak();

    // syscalls should use this task's stack to allow switching tasks from a syscall
    tls.kernel_stack.store(
        kernel_syscall_stack.as_mut_ptr_range().end,
        Ordering::Release,
    );

    // IA32_STAR : 0xC0000081
    Star::write(
        selectors.user_code,
        selectors.user_data,
        selectors.kernel_code,
        selectors.kernel_data,
    )
    .expect("IA32_STAR write incorrect");

    // syscall handler addr
    // IA32_LSTAR : 0xC0000082
    LStar::write(VirtAddr::new(handler.0 as u64));

    // disable interrupts on syscall
    // IA32_LSTAR : 0xC0000084
    SFMask::write(RFlags::INTERRUPT_FLAG /* | RFlags::TRAP_FLAG */);

    // enable syscall, sysret, systenter, sysexit
    // IA32_EFER : 0xC0000080
    unsafe {
        Efer::update(|flags| {
            flags.insert(EferFlags::SYSTEM_CALL_EXTENSIONS);
        });
    }
}

//

#[derive(Clone)]
pub struct FxRegs([u32; 128]);

impl fmt::Debug for FxRegs {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("FxRegs(..)")
    }
}

#[allow(unused)]
#[repr(C, align(16))]
#[derive(Debug, Clone)]
pub struct SyscallRegs {
    fxsave_reg: FxRegs,
    _r15: u64,
    _r14: u64,
    _r13: u64,
    _r12: u64,
    pub rflags: u64, // r11
    _r10: u64,
    pub arg4: u64, // r9
    pub arg3: u64, // r8
    _rbp: u64,
    pub arg1: u64,           // rsi
    pub arg0: u64,           // rdi
    pub arg2: u64,           // rdx
    pub user_instr_ptr: u64, // rcx
    _rbx: u64,
    pub syscall_id: u64,     // rax, also the return register
    pub user_stack_ptr: u64, // rsp
}

impl fmt::Display for SyscallRegs {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        writeln!(
            f,
            "syscall: {}, args: {:?}",
            self.syscall_id,
            (self.arg0, self.arg1, self.arg2, self.arg3, self.arg4)
        )
    }
}

impl SyscallRegs {
    pub fn new(ip: u64, sp: u64) -> Self {
        let mut mxcsr = mxcsr::read();
        // ignore exceptions from inexact float ops, why is it even a thing
        mxcsr.insert(
            MxCsr::INVALID_OPERATION_MASK
                | MxCsr::DENORMAL_MASK
                | MxCsr::DIVIDE_BY_ZERO_MASK
                | MxCsr::OVERFLOW_MASK
                | MxCsr::UNDERFLOW_MASK
                | MxCsr::PRECISION_MASK,
        );
        let mut fxsave_reg = [0u32; 128];
        fxsave_reg[6] = mxcsr.bits();

        Self {
            fxsave_reg: FxRegs(fxsave_reg),
            _r15: 0,
            _r14: 0,
            _r13: 0,
            _r12: 0,
            rflags: RFlags::INTERRUPT_FLAG.bits(),
            _r10: 0,
            arg4: 0,
            arg3: 0,
            _rbp: 0,
            arg1: 0,
            arg0: 0,
            arg2: 0,
            user_instr_ptr: ip,
            _rbx: 0,
            syscall_id: 0,
            user_stack_ptr: sp,
        }
    }

    #[naked]
    #[no_mangle]
    pub extern "sysv64" fn enter(&mut self) -> ! {
        // rdi = _args
        unsafe {
            naked_asm!(
                "mov rsp, rdi", // set the stack ptr to point to _args temporarily

                // load FPU/SSE/MMX state
                "fxrstor64 [rsp]",
                "add rsp, 512",

                // load registers
                "pop r15",
                "pop r14",
                "pop r13",
                "pop r12",
                "pop r11",
                "pop r10",
                "pop r9",
                "pop r8",
                "pop rbp",
                "pop rsi",
                "pop rdi",
                "pop rdx",
                "pop rcx",
                "pop rbx",
                "pop rax",

                "swapgs",
                "pop QWORD PTR gs:{user_stack}",
                "mov rsp, gs:{user_stack}",
                "swapgs",
                // TODO: fix the sysret vulnerability
                "sysretq",
                user_stack = const(ThreadLocalStorage::USER_STACK),
            );
        }
    }
}

//

pub struct SyscallHandler(usize);

impl SyscallHandler {
    /// # Safety
    /// Extremely unsafe, because the syscall enters kernel code
    /// with a user controlled stack with kernel privileges
    ///
    /// The stack has to be swapped to something trusted
    pub unsafe fn new(f: unsafe extern "C" fn()) -> Self {
        Self(f as usize)
    }
}

#[macro_export]
macro_rules! generate_handler {
    ($($t:tt)*) => {{
        #[naked]
        unsafe extern "C" fn syscall_wrapper() {
            // the stack is still the userland stack
            //
            // rcx = return address
            // rsp = user stack
            // r11 = rflags
            unsafe {
                core::arch::naked_asm!(
                    "swapgs", // swap gs and kernelgs to open up a few temporary data locations
                    "mov gs:{user_stack}, rsp",   // backup the user stack
                    "mov rsp, gs:{kernel_stack}", // switch to the kernel stack
                    "push QWORD PTR gs:{user_stack}",
                    "swapgs",

                    // save registers
                    "push rax",
                    "push rbx",
                    "push rcx",
                    "push rdx",
                    "push rdi",
                    "push rsi",
                    "push rbp",
                    "push r8",
                    "push r9",
                    "push r10",
                    "push r11",
                    "push r12",
                    "push r13",
                    "push r14",
                    "push r15",

                    // save FPU/SSE/MMX state
                    "sub rsp, 512",
                    "fxsave64 [rsp]",

                    // call the real syscall handler
                    "mov rdi, rsp",
                    "call {syscall}",
                    // return

                    // load FPU/SSE/MMX state
                    "fxrstor64 [rsp]",
                    "add rsp, 512",

                    // load registers
                    "pop r15",
                    "pop r14",
                    "pop r13",
                    "pop r12",
                    "pop r11",
                    "pop r10",
                    "pop r9",
                    "pop r8",
                    "pop rbp",
                    "pop rsi",
                    "pop rdi",
                    "pop rdx",
                    "pop rcx",
                    "pop rbx",
                    "pop rax",

                    "swapgs",
                    "pop QWORD PTR gs:{user_stack}",
                    "mov rsp, gs:{user_stack}",
                    "swapgs",
                    // TODO: fix the sysret vulnerability
                    "sysretq",
                    syscall = sym syscall,
                    user_stack =   const($crate::tls::ThreadLocalStorage::USER_STACK),
                    kernel_stack = const($crate::tls::ThreadLocalStorage::KERNEL_STACK),
                );
            }
        }

        #[inline(always)]
        unsafe extern "C" fn syscall(regs: *mut $crate::syscall::SyscallRegs) {
            ($($t)*)(unsafe { &mut *regs });
        }

        unsafe { $crate::syscall::SyscallHandler::new(syscall_wrapper) }
    }};
}
