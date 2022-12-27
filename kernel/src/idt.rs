use x86_64::structures::idt::{InterruptDescriptorTable, InterruptStackFrame};

pub fn init() {
    let mut idt = InterruptDescriptorTable::new();
    idt.breakpoint.set_handler_fn(breakpoint_handler);
}

extern "x86-interrupt" fn breakpoint_handler(stack_frame: InterruptStackFrame) {}
