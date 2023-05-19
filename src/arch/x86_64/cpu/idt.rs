use super::{ints::*, tss::Tss};
use x86_64::structures::idt::{InterruptDescriptorTable, InterruptStackFrame};

//

#[derive(Debug, Clone, Copy)]
#[repr(u8)]
pub enum Irq {
    // BEG: 0x20..0x30 PIC space
    PicTimer = 0x20, // aka. the PIT (Programmable Interrupt Timer)
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

#[cfg(test)]
mod tests {
    #[test_case]
    fn breakpoint() {
        // breakpoint instruction
        x86_64::instructions::interrupts::int3();
    }
}
