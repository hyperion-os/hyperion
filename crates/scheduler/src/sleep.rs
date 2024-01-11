use alloc::sync::Arc;
use core::{
    sync::atomic::{AtomicBool, Ordering},
    task::Context,
};

use futures_util::{
    task::{waker, ArcWake},
    FutureExt,
};
use hyperion_instant::Instant;

use crate::{Task, READY};

//

struct SleepWaker {
    task: Task,
    deadline: Instant,
    once: AtomicBool,
}

impl ArcWake for SleepWaker {
    fn wake_by_ref(arc_self: &Arc<Self>) {
        if arc_self.deadline.is_reached() {
            if arc_self.once.swap(false, Ordering::Acquire) {
                READY.push(arc_self.task.clone());
            } else {
                panic!("SleepWaker double wake");
            }
        } else {
            unreachable!()
        }
    }
}

//

pub fn push(deadline: Instant, task: Task) {
    let mut fut = hyperion_events::timer::sleep_until(deadline);

    let waker = waker(Arc::new(SleepWaker {
        task,
        deadline,
        once: AtomicBool::new(false),
    }));
    let mut cx = Context::from_waker(&waker);
    if fut.poll_unpin(&mut cx).is_ready() {
        cx.waker().wake_by_ref();
    }
}
