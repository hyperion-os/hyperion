use core::{
    arch::asm,
    sync::atomic::{AtomicPtr, Ordering},
};

use memoffset::offset_of;
use x86_64::{
    registers::{
        model_specific::{Efer, EferFlags, GsBase, KernelGsBase, LStar, SFMask, Star},
        rflags::RFlags,
    },
    structures::idt::InterruptStackFrame,
    VirtAddr,
};

use crate::{cpu::gdt::SegmentSelectors, tls::ThreadLocalStorage};

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

#[no_mangle]
pub unsafe extern "sysv64" fn userland(_instr_ptr: VirtAddr, _stack_ptr: VirtAddr) -> ! {
    // rdi = _instr_ptr
    // rsi = _stack_ptr
    asm!(
        "cli",
        "swapgs",
        "mov rcx, rdi", // RDI = _instr_ptr
        "mov rsp, rsi", // RSI = _stack_ptr
        "mov r11, {rflags}",
        // clear some registers
        "xor rax, rax",
        "xor rbx, rbx",
        // no zeroing rcx, sysreq returns to the address in it (`instr_ptr`)
        "xor rdx, rdx",
        "xor rdi, rdi",
        "xor rsi, rsi",
        "xor rbp, rbp",
        // no zeroing rsp, a stack is needed
        "xor r8, r8",
        "xor r9, r9",
        "xor r10, r10",
        // no zeroing r11, it holds RFLAGS
        "xor r12, r12",
        "xor r13, r13",
        "xor r14, r14",
        "xor r15, r15",
        // "call {halt}",
        "sysretq",
        rflags = const(RFlags::INTERRUPT_FLAG.bits() /* | RFlags::TRAP_FLAG.bits() */),
        options(noreturn)
    )
}

//

#[naked]
unsafe extern "C" fn syscall_wrapper() {
    // the stack is still the userland stack
    //
    // rcx = return address
    // rsp = user stack
    // r11 = rflags
    asm!(

        "swapgs", // swap gs and kernelgs to open up a few temporary data locations
        "mov gs:{user_stack}, rsp",   // backup the user stack
        "mov rsp, gs:{kernel_stack}", // switch to the kernel stack

        "push QWORD PTR gs:{user_stack}",

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

        "call {syscall}",

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

        "pop QWORD PTR gs:{user_stack}",

        "mov rsp, gs:{user_stack}",
        "swapgs",
        // TODO: fix the sysret vulnerability
        "sysretq",
        syscall = sym syscall,
        user_stack = const(offset_of!(ThreadLocalStorage, user_stack)),
        kernel_stack = const(offset_of!(ThreadLocalStorage, kernel_stack)),
        options(noreturn)
    );
}

#[no_mangle]
extern "C" fn syscall(rdi: u64, rsi: u64, rdx: u64, _rcx_ignored: u64, r8: u64, r9: u64) {
    hyperion_log::debug!("got syscall {rdi} {rsi} {rdx} {r8} {r9}");
}
