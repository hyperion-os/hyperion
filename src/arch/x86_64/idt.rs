use crate::{debug, error, info};
use spin::Lazy;
use x86_64::{
    registers::control::Cr2,
    structures::idt::{InterruptDescriptorTable, InterruptStackFrame, PageFaultErrorCode},
};

//

pub static DOUBLE_FAULT_IST: u16 = 1;

//

pub fn init() {
    debug!("Initializing IDT");
    IDT.load();
}

//

extern "x86-interrupt" fn breakpoint(stack: InterruptStackFrame) {
    info!("INT: Breakpoint\n{stack:#?}")
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

    crate::qemu::_print(format_args_nl!("INT: Double fault ({ec})\n{stack:#?}"));

    let sp = stack.stack_pointer.as_ptr() as *const [u8; 8];
    for i in 0isize..256 {
        let sp = unsafe { sp.offset(i) };
        let bytes: [u8; 8] = unsafe { *sp };
        let graphic = |c: u8| {
            if c.is_ascii_graphic() {
                c as char
            } else {
                '.'
            }
        };
        crate::qemu::_print(format_args_nl!(
            "{:#x}:  {:02x} {:02x} {:02x} {:02x}  {:02x} {:02x} {:02x} {:02x}   {}{}{}{}{}{}{}{}",
            sp as usize,
            bytes[0],
            bytes[1],
            bytes[2],
            bytes[3],
            bytes[4],
            bytes[5],
            bytes[6],
            bytes[7],
            graphic(bytes[0]),
            graphic(bytes[1]),
            graphic(bytes[2]),
            graphic(bytes[3]),
            graphic(bytes[4]),
            graphic(bytes[5]),
            graphic(bytes[6]),
            graphic(bytes[7]),
        ));
    }

    panic!();
}

extern "x86-interrupt" fn page_fault(stack: InterruptStackFrame, ec: PageFaultErrorCode) {
    let addr = Cr2::read();

    error!("INT: Page fault\nAddress: {addr:?}\nErrorCode: {ec:?}\n{stack:#?}");

    panic!();
}

//

static IDT: Lazy<InterruptDescriptorTable> = Lazy::new(|| {
    let mut idt = InterruptDescriptorTable::new();

    idt.breakpoint.set_handler_fn(breakpoint);

    let opt = idt.double_fault.set_handler_fn(double_fault);
    unsafe {
        opt.set_stack_index(DOUBLE_FAULT_IST);
    }

    idt.page_fault.set_handler_fn(page_fault);

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
