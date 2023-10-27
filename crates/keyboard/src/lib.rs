#![no_std]

//

use core::task::{Context, Poll};

use crossbeam::atomic::AtomicCell;
use crossbeam_queue::ArrayQueue;
use futures_util::task::AtomicWaker;
use hyperion_int_safe_lazy::IntSafeLazy;
use hyperion_log::warn;

use self::event::KeyboardEvent;

//

mod decode;
pub mod event;

//

pub static LAZY: AtomicCell<fn()> = AtomicCell::new(noop);

//

pub fn provide_raw_keyboard_event(ps2_byte: u8) {
    let Some(event) = decode::process(ps2_byte) else {
        return;
    };

    provide_keyboard_event(event);
}

pub fn provide_keyboard_event(event: KeyboardEvent) {
    let Some(queue) = KEYBOARD_EVENT_QUEUE.get() else {
        warn!("Keyboard event queue not initialized! Lost '{event:?}'");
        return;
    };

    if let Some(old) = queue.force_push(event) {
        warn!("Keyboard event queue full! Lost '{old:?}'");
    }

    KEYBOARD_EVENT_WAKER.wake()
}

pub fn next_keyboard_event() -> Option<KeyboardEvent> {
    run_lazy();
    KEYBOARD_EVENT_QUEUE.get_force().pop()
}

pub fn wait_keyboard_event(cx: &mut Context) -> Poll<KeyboardEvent> {
    run_lazy();
    let queue = KEYBOARD_EVENT_QUEUE.get_force();

    if let Some(ev) = queue.pop() {
        return Poll::Ready(ev);
    }

    // need to check if a keyboard event happened right here

    KEYBOARD_EVENT_WAKER.register(cx.waker());

    // .. with this
    if let Some(ev) = queue.pop() {
        KEYBOARD_EVENT_WAKER.take();
        Poll::Ready(ev)
    } else {
        Poll::Pending
    }
}

pub fn force_init_queue() {
    KEYBOARD_EVENT_QUEUE.get_force();
}

pub use decode::{layouts, set_layout};

//

static KEYBOARD_EVENT_QUEUE: IntSafeLazy<ArrayQueue<KeyboardEvent>> =
    IntSafeLazy::new(|| ArrayQueue::new(512));
static KEYBOARD_EVENT_WAKER: AtomicWaker = AtomicWaker::new();

//

fn noop() {}

fn run_lazy() {
    LAZY.swap(noop)();
}
