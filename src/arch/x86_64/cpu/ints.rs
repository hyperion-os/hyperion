use x86_64::{
    instructions::port::Port,
    registers::control::Cr2,
    structures::idt::{InterruptStackFrame, PageFaultErrorCode},
};

use crate::{
    driver::{
        self,
        acpi::{apic::apic_regs, RegWrite},
        pic::PICS,
        rtc::RTC,
    },
    error, info,
    scheduler::tick::provide_tick,
};

use super::idt::Irq;

//

pub extern "x86-interrupt" fn pic_timer(_: InterruptStackFrame) {
    /*     info!("pit int"); */
    provide_tick();
    PICS.lock().end_of_interrupt(Irq::PicTimer as _);
}

pub extern "x86-interrupt" fn keyboard(_: InterruptStackFrame) {
    let scancode: u8 = unsafe { Port::new(0x60).read() };
    driver::ps2::keyboard::process(scancode);
    /*     info!("keyboard input"); */

    PICS.lock().end_of_interrupt(Irq::PicKeyboard as _);
}

pub extern "x86-interrupt" fn rtc_tick(_: InterruptStackFrame) {
    info!("RTC tick");
    provide_tick();
    RTC.int_ack();
    PICS.lock().end_of_interrupt(Irq::PicRtc as _);
}

pub extern "x86-interrupt" fn apic_timer(_: InterruptStackFrame) {
    provide_tick();
    apic_regs().eoi.write(0);
}

pub extern "x86-interrupt" fn apic_spurious(_: InterruptStackFrame) {
    provide_tick();
}

pub extern "x86-interrupt" fn breakpoint(stack: InterruptStackFrame) {
    info!("INT: Breakpoint\n{stack:#?}")
}

pub extern "x86-interrupt" fn double_fault(stack: InterruptStackFrame, ec: u64) -> ! {
    error!("INT: Double fault ({ec})\n{stack:#?}");

    panic!();
}

pub extern "x86-interrupt" fn page_fault(stack: InterruptStackFrame, ec: PageFaultErrorCode) {
    let addr = Cr2::read();

    error!("INT: Page fault\nAddress: {addr:?}\nErrorCode: {ec:?}\n{stack:#?}");

    panic!();
}

pub extern "x86-interrupt" fn general_protection_fault(stack: InterruptStackFrame, e: u64) {
    let addr = Cr2::read();
    error!("INT: General Protection Fault\nAddress: {addr:?}\ne: {e:#x}\n{stack:#?}");

    panic!();
}
