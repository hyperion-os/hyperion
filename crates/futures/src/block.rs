use alloc::sync::Arc;
use core::{
    future::{Future, IntoFuture},
    sync::atomic::{AtomicBool, Ordering},
    task::{Context, Poll},
};

use futures_util::{
    pin_mut,
    task::{waker, ArcWake},
};

// use hyperion_scheduler::futex;
use crate::executor::run_once;

//

// run a task to completion
pub fn block_on<F: IntoFuture>(f: F) -> F::Output {
    let fut = f.into_future();
    pin_mut!(fut);

    let wake = Arc::new(BlockOn {
        wake: AtomicBool::new(false),
    });
    let waker = waker(wake.clone());
    let mut cx = Context::from_waker(&waker);

    loop {
        debug_assert!(!wake.wake.load(Ordering::SeqCst));
        if let Poll::Ready(res) = fut.as_mut().poll(&mut cx) {
            return res;
        }

        // run other tasks while this task is waiting
        loop {
            // hyperion_log::debug!("block_on run_once");

            while run_once().is_some() {}

            if wake
                .wake
                .compare_exchange(true, false, Ordering::Acquire, Ordering::Relaxed)
                .is_ok()
            {
                break;
            }

            // no tasks and the block_on future is not ready
            // disable interrupts and wait for the next interrupt
            // (interrupts are the only way any task can become ready to poll)
            //
            // TODO: inter-processor interrupts to wake up one block_on task
            // that is waiting here, but another CPU sends some data and wakes this up
            // currently the block_on task would eventually wake up
            // from the next APIC timer interrupt
            hyperion_arch::int::wait();

            if wake
                .wake
                .compare_exchange(true, false, Ordering::Acquire, Ordering::Relaxed)
                .is_ok()
            {
                break;
            }
        }
    }
}

//

struct BlockOn {
    wake: AtomicBool,
}

impl ArcWake for BlockOn {
    fn wake_by_ref(arc_self: &Arc<Self>) {
        // arc_self.wake.store(1, Ordering::Release);
        // futex::wake(&arc_self.wake, 1);
        arc_self.wake.store(true, Ordering::Release);
        // arc_self.notify.notify_one();
    }
}
