use super::tss::Tss;
use crate::{
    driver::{self, acpi::apic::apic_regs, pic::PICS},
    error, info,
};
use x86_64::{
    instructions::port::Port,
    registers::control::Cr2,
    structures::idt::{InterruptDescriptorTable, InterruptStackFrame, PageFaultErrorCode},
};

//

pub const PIC_IRQ_OFFSET: u8 = 32;
pub const PIC_TIMER_IRQ: u8 = PIC_IRQ_OFFSET;
pub const KEYBOARD_IRQ: u8 = PIC_IRQ_OFFSET + 1;
pub const RTC_IRQ: u8 = PIC_IRQ_OFFSET + 8;

pub const TIMER_IRQ: u8 = 0x32;
pub const SPURIOUS_IRQ: u8 = 0xFF;

//

pub struct Idt {
    inner: InterruptDescriptorTable,
}

//

impl Idt {
    pub fn new(tss: &Tss) -> Self {
        let mut idt = InterruptDescriptorTable::new();

        idt[PIC_TIMER_IRQ as _].set_handler_fn(pic_timer);
        idt[KEYBOARD_IRQ as _].set_handler_fn(keyboard);
        idt[RTC_IRQ as _].set_handler_fn(rtc_tick);
        idt[TIMER_IRQ as _].set_handler_fn(apic_timer);
        idt[SPURIOUS_IRQ as _].set_handler_fn(apic_spurious);

        idt.breakpoint.set_handler_fn(breakpoint);

        let opt = idt.double_fault.set_handler_fn(double_fault);
        let stack = tss
            .stacks
            .take_interrupt_stack()
            .expect("Out of interrupt stacks");
        unsafe {
            opt.set_stack_index(stack);
        }

        idt.page_fault.set_handler_fn(page_fault);

        idt.general_protection_fault
            .set_handler_fn(general_protection_fault);

        Self { inner: idt }
    }

    pub fn load(&'static self) {
        // trace!("Loading IDT");
        self.inner.load()
    }
}

//

pub extern "x86-interrupt" fn pic_timer(_: InterruptStackFrame) {
    // info!(".");
    PICS.lock().end_of_interrupt(PIC_TIMER_IRQ);
}

pub extern "x86-interrupt" fn keyboard(_: InterruptStackFrame) {
    let scancode: u8 = unsafe { Port::new(0x60).read() };
    if let Some(ch) = driver::ps2::process(scancode) {
        info!("{ch}");
    }

    PICS.lock().end_of_interrupt(KEYBOARD_IRQ);
}

pub extern "x86-interrupt" fn rtc_tick(_: InterruptStackFrame) {
    info!("RTC tick");
    PICS.lock().end_of_interrupt(RTC_IRQ);
}

pub extern "x86-interrupt" fn apic_timer(_: InterruptStackFrame) {
    apic_regs().eoi.write(0);
}

pub extern "x86-interrupt" fn apic_spurious(_: InterruptStackFrame) {}

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

//

#[cfg(test)]
mod tests {
    #[test_case]
    fn breakpoint() {
        // breakpoint instruction
        x86_64::instructions::interrupts::int3();
    }
}
