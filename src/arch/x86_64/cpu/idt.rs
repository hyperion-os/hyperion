use x86_64::structures::idt::{InterruptDescriptorTable, InterruptStackFrame};

use super::{ints::*, tss::Tss};

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

        idt.divide_error.set_handler_fn(divide_error);
        idt.debug.set_handler_fn(debug);
        idt.non_maskable_interrupt
            .set_handler_fn(non_maskable_interrupt);
        idt.breakpoint.set_handler_fn(breakpoint);
        idt.overflow.set_handler_fn(overflow);
        idt.bound_range_exceeded
            .set_handler_fn(bound_range_exceeded);
        idt.invalid_opcode.set_handler_fn(invalid_opcode);
        idt.device_not_available
            .set_handler_fn(device_not_available);
        let opt = idt.double_fault.set_handler_fn(double_fault);
        unsafe {
            opt.set_stack_index(
                tss.stacks
                    .take_interrupt_stack()
                    .expect("Out of interrupt stacks"),
            );
        }
        idt.invalid_tss.set_handler_fn(invalid_tss);
        idt.segment_not_present.set_handler_fn(segment_not_present);
        idt.stack_segment_fault.set_handler_fn(stack_segment_fault);
        idt.general_protection_fault
            .set_handler_fn(general_protection_fault);
        idt.page_fault.set_handler_fn(page_fault);
        idt.x87_floating_point.set_handler_fn(x87_floating_point);
        idt.alignment_check.set_handler_fn(alignment_check);
        idt.machine_check.set_handler_fn(machine_check);
        idt.simd_floating_point.set_handler_fn(simd_floating_point);
        idt.virtualization.set_handler_fn(virtualization);
        idt.vmm_communication_exception
            .set_handler_fn(vmm_communication_exception);
        idt.security_exception.set_handler_fn(security_exception);

        for irq in Irq::iter() {
            idt[irq as _].set_handler_fn(irq.handler());
        }

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
