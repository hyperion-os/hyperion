use core::{
    arch::{asm, naked_asm},
    fmt,
    mem::offset_of,
};

use crossbeam::atomic::AtomicCell;
use x86_64::{
    registers::{
        model_specific::{Efer, EferFlags, LStar, SFMask, Star},
        mxcsr::{self, MxCsr},
        rflags::RFlags,
    },
    PhysAddr, VirtAddr,
};

use crate::{cpu::gdt::SegmentSelectors, tls::ThreadLocalStorage, vmm::PageMap};

//

/// init `syscall` and `sysret`
pub fn init(selectors: SegmentSelectors) {
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
    LStar::write(VirtAddr::new(syscall_wrapper as usize as u64));

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

pub fn set_handler(f: fn(&mut SyscallRegs)) {
    SYSCALL_HANDLER.store(f);
}

//

#[allow(unused)]
#[repr(C)]
#[derive(Debug, Clone)]
pub struct SyscallRegs {
    // fs: ,
    cr3: PhysAddr,
    fxsave_reg: [u32; 128],

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
    pub fn new(ip: u64, sp: u64, page_map: &PageMap) -> Self {
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

        let cr3 = page_map.cr3().start_address();

        Self {
            cr3,
            fxsave_reg,
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

                // load address space
                "pop rax", // pop cr3 into a temprary register
                "mov rcx, cr3", // rcx = prev virtual address space
                "cmp rax, rcx", // cmp for if
                "je 2f",         // if rax != rcx:
                "mov cr3, rax", // load next virtual address space
                // writing cr3, even if the value is the same, triggers a TLB flush (which is bad)
                "2:",

                // load FPU/SSE/MMX state
                "add rsp, 512",
                "fxrstor64 [rsp]",

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
                user_stack = const(offset_of!(ThreadLocalStorage, user_stack)),
            );
        }
    }
}

//

#[naked]
unsafe extern "C" fn syscall_wrapper() {
    // the stack is still the userland stack
    //
    // rcx = return address
    // rsp = user stack
    // r11 = rflags
    unsafe {
        naked_asm!(
            "cli",
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
            "fxsave64 [rsp]",
            "sub rsp, 512",

            // save address space
            "mov rcx, cr3",
            "push rcx",

            // call the real syscall handler
            "mov rdi, rsp",
            "call {syscall}",
            // return

            // load address space
            "pop rax", // pop cr3 into a temprary register
            "mov rcx, cr3", // rcx = prev virtual address space
            "cmp rax, rcx", // cmp for if
            "je 2f",         // if rax != rcx:
            "mov cr3, rax", // load next virtual address space
            // writing cr3, even if the value is the same, triggers a TLB flush (which is bad)
            "2:",

            // load FPU/SSE/MMX state
            "add rsp, 512",
            "fxrstor64 [rsp]",

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
            user_stack = const(offset_of!(ThreadLocalStorage, user_stack)),
            kernel_stack = const(offset_of!(ThreadLocalStorage, kernel_stack))
        );
    }
}

#[inline(always)]
unsafe extern "C" fn syscall(regs: *mut SyscallRegs) {
    SYSCALL_HANDLER.load()(unsafe { &mut *regs });
}

// TODO: static linking instead of dynamic fn ptr
static SYSCALL_HANDLER: AtomicCell<fn(&mut SyscallRegs)> = AtomicCell::new(|_| {
    hyperion_log::error!("Syscall handler not initialized");
});
