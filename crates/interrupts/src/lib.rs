#![no_std]

//

use crossbeam::atomic::AtomicCell;

//

pub static INT_ALLOCATOR: () = ();
pub static INT_CONTROLLER: AtomicCell<IntController> = AtomicCell::new(IntController::Pic);

//

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum IntController {
    /// The legacy 'Programmable Interrupt Timer'
    Pic,

    /// 'Advanced Programmable Interrupt Timer'
    Apic,
}

const _: () = assert!(AtomicCell::<IntController>::is_lock_free());
