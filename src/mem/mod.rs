use crate::{boot, debug, mem::map::Memmap, util::fmt::NumberPostfix};

//

pub mod map;

// allocator
pub mod bump;
pub mod pfa;

//

pub fn init() {
    let usable = boot::memmap().map(|Memmap { len, .. }| len).sum::<u64>();
    let total = boot::memtotal();
    debug!("Usable system memory: {}B", usable.postfix_binary());
    debug!("Total system memory: {}B", total.postfix_binary());

    bump::init();
    pfa::init();
}
