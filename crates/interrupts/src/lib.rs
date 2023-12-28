#![no_std]

//

use core::sync::atomic::{AtomicBool, Ordering};

use crossbeam::atomic::AtomicCell;
use hyperion_macros::array;

//

pub const INT_COUNT: usize = 0x100 - 0x20;

pub static INT_ALLOCATOR: () = ();
pub static INT_CONTROLLER: AtomicCell<IntController> = AtomicCell::new(IntController::None);
pub static INT_EOI_HANDLER: AtomicCell<fn(u8)> = AtomicCell::new(|_| {});
pub static INT_HANDLERS: [IntHandler; INT_COUNT] = array![IntHandler::new(); 224];

//

pub fn set_any_interrupt_handler(can_use: impl Fn(u8) -> bool, f: fn(u8, usize)) -> Option<u8> {
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

pub fn set_interrupt_handler_if_free(irq: u8, f: fn(u8, usize)) -> bool {
    handler(irq).store_if_free(f)
}

pub fn set_interrupt_handler(irq: u8, f: fn(u8, usize)) {
    handler(irq).store(f)
}

pub fn handler(irq: u8) -> &'static IntHandler {
    &INT_HANDLERS[irq as usize - 0x20]
}

pub fn interrupt_handler(irq: u8, ip: usize) {
    // debug!("interrupt {irq}");
    INT_HANDLERS[irq as usize - 0x20].load()(irq, ip);
    // end_of_interrupt(irq);
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

pub fn default_handler(irq: u8, _ip: usize) {
    end_of_interrupt(irq)
}

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

pub struct IntHandler {
    free: AtomicBool,
    f: AtomicCell<fn(u8, usize)>,
}

//

impl IntHandler {
    pub const fn new() -> Self {
        Self {
            free: AtomicBool::new(true),
            f: AtomicCell::new(default_handler),
        }
    }

    pub fn store_if_free(&self, new: fn(u8, usize)) -> bool {
        let stored = self.free.swap(false, Ordering::SeqCst);
        if stored {
            self.f.store(new);
        }
        stored
    }

    pub fn store(&self, new: fn(u8, usize)) {
        self.free.store(false, Ordering::SeqCst);
        self.f.store(new);
    }

    pub fn load(&self) -> fn(u8, usize) {
        self.f.load()
    }
}

//

const _: () = assert!(AtomicCell::<IntController>::is_lock_free());
const _: () = assert!(AtomicCell::<fn(u8)>::is_lock_free());
const _: () = assert!(AtomicCell::<fn()>::is_lock_free());
