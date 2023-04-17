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

#[derive(Debug, Clone, Copy)]
#[repr(u8)]
pub enum Irq {
    // BEG: 0x20..0x30 PIC space
    PicTimer = 0x20,
    PicKeyboard = 0x21,
    PicRtc = 0x28,
    // END: 0x20..0x30 PIC space
    // BEG: 0x30..0xFF APIC space
    ApicTimer = 0x32,
    ApicSpurious = 0xFF,
    // END: 0x30..0xFF APIC space
}

pub struct Idt {
    inner: InterruptDescriptorTable,
}

//

impl Irq {
    pub fn iter() -> impl DoubleEndedIterator + ExactSizeIterator<Item = Self> {
        [
            Self::PicTimer,
            Self::PicKeyboard,
            Self::PicRtc,
            Self::ApicTimer,
            Self::ApicSpurious,
        ]
        .into_iter()
    }

    pub fn handler(self) -> extern "x86-interrupt" fn(InterruptStackFrame) {
        match self {
            Irq::PicTimer => pic_timer,
            Irq::PicKeyboard => keyboard,
            Irq::PicRtc => rtc_tick,
            Irq::ApicTimer => apic_timer,
            Irq::ApicSpurious => apic_spurious,
        }
    }
}

impl Idt {
    pub fn new(tss: &Tss) -> Self {
        let mut idt = InterruptDescriptorTable::new();

        for irq in Irq::iter() {
            idt[irq as _].set_handler_fn(irq.handler());
        }

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
    PICS.lock().end_of_interrupt(Irq::PicTimer as _);
}

pub extern "x86-interrupt" fn keyboard(_: InterruptStackFrame) {
    let scancode: u8 = unsafe { Port::new(0x60).read() };
    if let Some(ch) = driver::ps2::keyboard::process(scancode) {
        info!("{ch}");
    }

    PICS.lock().end_of_interrupt(Irq::PicKeyboard as _);
}

pub extern "x86-interrupt" fn rtc_tick(_: InterruptStackFrame) {
    info!("RTC tick");
    PICS.lock().end_of_interrupt(Irq::PicRtc as _);
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
