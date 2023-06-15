#![no_std]

//

use core::task::{Context, Poll};

use crossbeam::atomic::AtomicCell;
use crossbeam_queue::ArrayQueue;
use futures_util::task::AtomicWaker;
use hyperion_int_safe_lazy::IntSafeLazy;
use hyperion_log::warn;

//

pub static LAZY: AtomicCell<fn()> = AtomicCell::new(noop);

//

pub fn provide_keyboard_event(c: char) {
    let Some(queue) = KEYBOARD_EVENT_QUEUE.get() else {
        warn!("Keyboard event queue not initialized! Lost '{c}'");
        return
    };

    if let Some(old) = queue.force_push(c) {
        warn!("Keyboard event queue full! Lost '{old}'");
    }

    KEYBOARD_EVENT_WAKER.wake()
}

pub fn next_keyboard_event() -> Option<char> {
    run_lazy();
    KEYBOARD_EVENT_QUEUE.get_force().pop()
}

pub fn wait_keyboard_event(cx: &mut Context) -> Poll<char> {
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

fn noop() {}

fn run_lazy() {
    LAZY.swap(noop)();
}

//

static KEYBOARD_EVENT_QUEUE: IntSafeLazy<ArrayQueue<char>> =
    IntSafeLazy::new(|| ArrayQueue::new(256));
static KEYBOARD_EVENT_WAKER: AtomicWaker = AtomicWaker::new();
