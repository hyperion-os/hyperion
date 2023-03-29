use super::cpu::idt::{breakpoint, double_fault, page_fault};
use crate::{debug, trace};
use alloc::format;
use spin::Lazy;
use x86_64::structures::idt::InterruptDescriptorTable;

//

pub fn init() {
    debug!("Initializing IDT");

    let b = format!("{:?}", *IDT);
    let b = b.as_bytes().array_chunks::<4>().fold(0, |acc, s| {
        let v = u32::from_ne_bytes(*s);
        acc ^ v
    });
    trace!("IDT hash: {b}");

    crate::qemu::_print(format_args_nl!("{:#?}", *IDT));
    IDT.load();
}

//

static IDT: Lazy<InterruptDescriptorTable> = Lazy::new(|| {
    let mut idt = InterruptDescriptorTable::new();

    idt.breakpoint.set_handler_fn(breakpoint);

    let opt = idt.double_fault.set_handler_fn(double_fault);
    // let stack = tss
    //     .stacks
    //     .take_interrupt_stack()
    //     .expect("Out of interrupt stacks");
    // unsafe {
    //     opt.set_stack_index(stack);
    // }

    idt.page_fault.set_handler_fn(page_fault);

    idt
});
