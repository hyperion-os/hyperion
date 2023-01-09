use crate::println;
use spin::Lazy;
use x86_64::structures::idt::{InterruptDescriptorTable, InterruptStackFrame};

//

pub static DOUBLE_FAULT_IST: u16 = 1;

//

pub fn init() {
    IDT.load();
}

//

extern "x86-interrupt" fn breakpoint(stack: InterruptStackFrame) {
    println!("INT: Breakpoint\n{stack:#?}")
}

extern "x86-interrupt" fn double_fault(stack: InterruptStackFrame, ec: u64) -> ! {
    // SAFETY: Unlocking the Mutex is safe if this is the only CPU running
    //
    // This CPU might have locked the COM1 writer and then stack-overflowed before unlocking it but
    // we won't return anyways, so lets just unlock it
    unsafe {
        // TODO: This won't be safe when multiple CPUs are running
        crate::qemu::unlock();
    }
    panic!("INT: Double fault ({ec})\n{stack:#?}")
}

//

static IDT: Lazy<InterruptDescriptorTable> = Lazy::new(|| {
    let mut idt = InterruptDescriptorTable::new();
    idt.breakpoint.set_handler_fn(breakpoint);
    unsafe {
        idt.double_fault
            .set_handler_fn(double_fault)
            .set_stack_index(DOUBLE_FAULT_IST);
    }
    idt
});

//

#[cfg(test)]
mod tests {
    #[test_case]
    fn breakpoint() {
        // breakpoint instruction
        x86_64::instructions::interrupts::int3();
    }
}
