use crossbeam::atomic::AtomicCell;
use hyperion_log::{error, info, trace};
use hyperion_mem::vmm::{Handled, NotHandled, PageFaultResult, PageMapImpl, Privilege};
use x86_64::{
    registers::control::Cr2,
    structures::idt::{InterruptStackFrame, PageFaultErrorCode},
};

use crate::vmm::PageMap;

//

pub static PAGE_FAULT_HANDLER: AtomicCell<fn(usize, usize, Privilege) -> PageFaultResult> =
    AtomicCell::new(|_, _, _| Ok(NotHandled));

pub static GP_FAULT_HANDLER: AtomicCell<fn()> = AtomicCell::new(|| {
    panic!();
});

//

pub extern "x86-interrupt" fn divide_error(stack: InterruptStackFrame) {
    error!("INT: Divide Error\n{stack:#?}");
    panic!();
}

pub extern "x86-interrupt" fn debug(stack: InterruptStackFrame) {
    info!("INT: Debug\n{stack:#?}");
}

pub extern "x86-interrupt" fn non_maskable_interrupt(stack: InterruptStackFrame) {
    error!("INT: Non Maskable Interrupt\n{stack:#?}");
    panic!();
}

pub extern "x86-interrupt" fn breakpoint(stack: InterruptStackFrame) {
    info!("INT: Breakpoint\n{stack:#?}")
}

pub extern "x86-interrupt" fn overflow(stack: InterruptStackFrame) {
    error!("INT: Overflow\n{stack:#?}");
    panic!();
}

pub extern "x86-interrupt" fn bound_range_exceeded(stack: InterruptStackFrame) {
    error!("INT: Bound Range Exceeded\n{stack:#?}");
    panic!();
}

pub extern "x86-interrupt" fn invalid_opcode(stack: InterruptStackFrame) {
    error!("INT: Invalid OpCode\n{stack:#?}");
    panic!();
}

pub extern "x86-interrupt" fn device_not_available(stack: InterruptStackFrame) {
    error!("INT: Device Not Available\n{stack:#?}");
    panic!();
}

pub extern "x86-interrupt" fn double_fault(stack: InterruptStackFrame, ec: u64) -> ! {
    error!("INT: Double fault ({ec})\n{stack:#?}");
    panic!();
}

pub extern "x86-interrupt" fn invalid_tss(stack: InterruptStackFrame, ec: u64) {
    error!("INT: Invalid TSS ({ec})\n{stack:#?}");
    panic!();
}

pub extern "x86-interrupt" fn segment_not_present(stack: InterruptStackFrame, ec: u64) {
    error!("INT: Segment Not Present ({ec})\n{stack:#?}");
    panic!();
}

pub extern "x86-interrupt" fn stack_segment_fault(stack: InterruptStackFrame, ec: u64) {
    error!("INT: Stack Segment Fault ({ec})\n{stack:#?}");
    panic!();
}

pub extern "x86-interrupt" fn general_protection_fault(stack: InterruptStackFrame, e: u64) {
    let addr = Cr2::read();

    error!("INT: General Protection Fault\nAddress: {addr:?}\ne: {e:#x}\n{stack:#?}");
    // unsafe { print_backtrace_from(stack.stack_pointer) };

    GP_FAULT_HANDLER.load()();
}

pub extern "x86-interrupt" fn page_fault(stack: InterruptStackFrame, ec: PageFaultErrorCode) {
    let addr = Cr2::read();

    trace!("INT: Page fault\nAddress: {addr:?}\nErrorCode: {ec:?}\n{stack:#?}");

    let privilege = if ec.contains(PageFaultErrorCode::USER_MODE) {
        Privilege::User
    } else {
        Privilege::Kernel
    };

    match (|| {
        PageMap::current().page_fault(addr, privilege)?;
        PAGE_FAULT_HANDLER.load()(
            stack.instruction_pointer.as_u64() as _,
            addr.as_u64() as _,
            privilege,
        )?;

        Ok(NotHandled)
    })() {
        Ok(NotHandled) => {
            error!("INT: Page fault\nAddress: {addr:?}\nErrorCode: {ec:?}\n{stack:#?}");
            panic!();
        }
        Err(Handled) => {
            trace!("page fault handled");
        }
    };
}

#[no_mangle]
pub extern "x86-interrupt" fn x87_floating_point(stack: InterruptStackFrame) {
    error!("INT: x87 Floating Point\n{stack:#?}");
    panic!();
}

pub extern "x86-interrupt" fn alignment_check(stack: InterruptStackFrame, ec: u64) {
    error!("INT: Alignment Check ({ec})\n{stack:#?}");
    panic!();
}

pub extern "x86-interrupt" fn machine_check(stack: InterruptStackFrame) -> ! {
    error!("INT: Machine Check\n{stack:#?}");
    panic!();
}

pub extern "x86-interrupt" fn simd_floating_point(stack: InterruptStackFrame) {
    error!("INT: SIMD Floating Point\n{stack:#?}");
    panic!();
}

pub extern "x86-interrupt" fn virtualization(stack: InterruptStackFrame) {
    error!("INT: Virtualization\n{stack:#?}");
    panic!();
}

pub extern "x86-interrupt" fn vmm_communication_exception(stack: InterruptStackFrame, ec: u64) {
    error!("INT: VMM Communication Exception ({ec})\n{stack:#?}");
    panic!();
}

pub extern "x86-interrupt" fn security_exception(stack: InterruptStackFrame, ec: u64) {
    error!("INT: Security Exception ({ec})\n{stack:#?}");
    panic!();
}

// other ints

pub mod other {
    use hyperion_interrupts::interrupt_handler;
    use x86_64::structures::idt::InterruptStackFrame;

    hyperion_macros::gen_int_handlers!("x86-interrupt");
}
