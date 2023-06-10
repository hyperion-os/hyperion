#![no_std]

//

use crossbeam_queue::ArrayQueue;
use futures_util::task::AtomicWaker;
use hyperion_int_safe_lazy::IntSafeLazy;
use hyperion_log::warn;

//

pub static KEYBOARD_EVENT_QUEUE: IntSafeLazy<ArrayQueue<char>> =
    IntSafeLazy::new(|| ArrayQueue::new(256));
pub static KEYBOARD_EVENT_WAKER: AtomicWaker = AtomicWaker::new();

//

pub fn provide_keyboard_event(c: char) {
    let Some(queue) = KEYBOARD_EVENT_QUEUE.get() else {
        return
    };

    if let Some(old) = queue.force_push(c) {
        warn!("Keyboard event queue full! Lost '{old}'");
    }

    KEYBOARD_EVENT_WAKER.wake()
}
