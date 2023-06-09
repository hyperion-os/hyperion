#![no_std]

//

use crossbeam::atomic::AtomicCell;
use hyperion_macros::array;

//

pub const INT_COUNT: usize = 0x100 - 0x20;

pub static INT_ALLOCATOR: () = ();
pub static INT_CONTROLLER: AtomicCell<IntController> = AtomicCell::new(IntController::None);
pub static INT_EOI_HANDLER: AtomicCell<fn(u8)> = AtomicCell::new(|_| {});
pub static INT_HANDLERS: [AtomicCell<fn()>; INT_COUNT] =
    array![AtomicCell::new(default_handler); 224];

//

pub fn set_any_interrupt_handler(can_use: impl Fn(u8) -> bool, f: fn()) -> Option<u8> {
    for irq in 0x20u8..=0xFF {
        if !can_use(irq) {
            continue;
        }

        if set_interrupt_handler_if_free(irq, f) {
            return Some(irq);
        }
    }

    None
}

pub fn set_interrupt_handler_if_free(irq: u8, f: fn()) -> bool {
    INT_HANDLERS[irq as usize - 0x20]
        .compare_exchange(default_handler, f)
        .is_ok()
}

pub fn set_interrupt_handler(irq: u8, f: fn()) -> bool {
    INT_HANDLERS[irq as usize - 0x20].swap(f) != default_handler
}

pub fn interrupt_handler(irq: u8) {
    // debug!("interrupt {irq}");
    INT_HANDLERS[irq as usize - 0x20].load()();
    end_of_interrupt(irq);
}

pub fn end_of_interrupt(irq: u8) {
    INT_EOI_HANDLER.load()(irq);
    /* match INT_CONTROLLER.load() {
        IntController::Pic => PICS.lock().end_of_interrupt(irq),
        IntController::Apic => {
            Lapic::current_mut().eoi();
        }
        IntController::None => {},
    } */
}

pub const fn default_handler() {}

//

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(u8)]
pub enum IntController {
    /// The legacy 'Programmable Interrupt Timer'
    Pic,

    /// 'Advanced Programmable Interrupt Timer'
    Apic,

    #[default]
    None,
}

const _: () = assert!(AtomicCell::<IntController>::is_lock_free());
const _: () = assert!(AtomicCell::<fn(u8)>::is_lock_free());
const _: () = assert!(AtomicCell::<fn()>::is_lock_free());
