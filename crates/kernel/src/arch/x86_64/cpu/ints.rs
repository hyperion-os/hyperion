use crossbeam::atomic::AtomicCell;
use hyperion_log::{error, info};
use x86_64::{
    instructions::port::Port,
    registers::control::Cr2,
    structures::idt::{InterruptStackFrame, PageFaultErrorCode},
};

use super::idt::Irq;
use crate::{
    backtrace::{self, print_backtrace_from},
    driver::{
        self,
        acpi::{apic::Lapic, hpet::HPET},
        pic::PICS,
        rtc::RTC,
    },
};

//

pub static INT_CONTROLLER: AtomicCell<IntController> = AtomicCell::new(IntController::Pic);

//

#[derive(Debug, Clone, Copy)]
#[repr(u8)]
pub enum IntController {
    Pic,
    Apic,
}

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
    no_inline(|| {
        let addr = Cr2::read();

        error!("INT: General Protection Fault\nAddress: {addr:?}\ne: {e:#x}\n{stack:#?}");
        unsafe { print_backtrace_from(stack.stack_pointer) };

        panic!();
    });
}

pub extern "x86-interrupt" fn page_fault(stack: InterruptStackFrame, ec: PageFaultErrorCode) {
    no_inline(|| {
        let addr = Cr2::read();

        error!("INT: Page fault\nAddress: {addr:?}\nErrorCode: {ec:?}\n{stack:#?}");
        unsafe { print_backtrace_from(stack.stack_pointer) };

        panic!();
    });
}

// emitting stack frames causes issues without this, SOMEHOW.. HOW.. WHAT
#[inline(never)]
pub fn no_inline(f: impl Fn()) {
    f()
}

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

pub extern "x86-interrupt" fn pic_timer(_: InterruptStackFrame) {
    /*     info!("pit int"); */
    eoi_irq(Irq::PicTimer as _);
}

pub extern "x86-interrupt" fn keyboard(f: InterruptStackFrame) {
    let scancode: u8 = unsafe { Port::new(0x60).read() };
    driver::ps2::keyboard::process(scancode);

    if driver::ps2::keyboard::debug_key() {
        unsafe { backtrace::print_backtrace_from(f.stack_pointer) };
    }

    eoi_irq(Irq::PicKeyboard as _);
}

pub extern "x86-interrupt" fn rtc_tick(_: InterruptStackFrame) {
    RTC.int_ack();
    eoi_irq(Irq::PicRtc as _);
}

pub extern "x86-interrupt" fn apic_timer(_: InterruptStackFrame) {
    eoi();
}

pub extern "x86-interrupt" fn hpet_sleep(_: InterruptStackFrame) {
    // crate::debug!("woke up at {}", HPET.main_counter_value());
    HPET.int_ack();
    eoi();
}

pub extern "x86-interrupt" fn apic_spurious(_: InterruptStackFrame) {
    // spurdo spärde keskeytys
    eoi();
}

//

fn eoi_irq(irq: u8) {
    match INT_CONTROLLER.load() {
        IntController::Pic => PICS.lock().end_of_interrupt(irq),
        IntController::Apic => {
            Lapic::current_mut().eoi();
        }
    }
}

fn eoi() {
    match INT_CONTROLLER.load() {
        IntController::Pic => unreachable!(),
        IntController::Apic => {
            Lapic::current_mut().eoi();
        }
    }
}

const _: () = assert!(AtomicCell::<IntController>::is_lock_free());
