use crate::debug;
use core::ops::Deref;
use spin::Lazy;
use x86_64::structures::idt::{InterruptDescriptorTable, InterruptStackFrame, PageFaultErrorCode};

//

pub fn init() {
    debug!("Initializing IDT");
    IDT.load();
}

//

static IDT: Lazy<InterruptDescriptorTable> = Lazy::new(|| {
    let mut idt = InterruptDescriptorTable::new();

    idt.breakpoint.set_handler_fn(super::cpu::idt::breakpoint);

    let opt = idt
        .double_fault
        .set_handler_fn(super::cpu::idt::double_fault);
    // unsafe {
    // opt.set_stack_index(0);
    // }

    idt.page_fault.set_handler_fn(super::cpu::idt::page_fault);

    idt
});
