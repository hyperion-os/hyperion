use x86_64::structures::idt::InterruptDescriptorTable;

use super::{ints::*, tss::Tss};

//

#[derive(Debug)]
pub struct Idt {
    inner: InterruptDescriptorTable,
}

//

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
        let opt = idt.page_fault.set_handler_fn(page_fault);
        unsafe {
            opt.set_stack_index(
                tss.stacks
                    .take_interrupt_stack()
                    .expect("Out of interrupt stacks"),
            );
        }
        idt.x87_floating_point.set_handler_fn(x87_floating_point);
        idt.alignment_check.set_handler_fn(alignment_check);
        idt.machine_check.set_handler_fn(machine_check);
        idt.simd_floating_point.set_handler_fn(simd_floating_point);
        idt.virtualization.set_handler_fn(virtualization);
        idt.vmm_communication_exception
            .set_handler_fn(vmm_communication_exception);
        idt.security_exception.set_handler_fn(security_exception);

        use super::ints::other::*;
        for (irq, handler) in hyperion_macros::get_int_handlers!() {
            idt[irq as usize].set_handler_fn(handler);
        }

        Self { inner: idt }
    }

    pub fn load(&'static self) {
        // trace!("Loading IDT");
        self.inner.load()
    }
}
