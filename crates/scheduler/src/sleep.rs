use alloc::sync::Arc;
use core::task::Context;

use futures_util::{
    task::{waker, ArcWake},
    FutureExt,
};
use hyperion_instant::Instant;
use hyperion_sync::TakeOnce;

use crate::{Task, READY};

//

struct SleepWaker {
    task: TakeOnce<Task>,

    // useless data just for an assert:
    #[cfg(debug_assertions)]
    deadline: Instant,
}

impl ArcWake for SleepWaker {
    fn wake_by_ref(arc_self: &Arc<Self>) {
        #[cfg(debug_assertions)]
        assert!(arc_self.deadline.is_reached());

        let Some(task) = arc_self.task.take() else {
            return;
        };

        READY.push(task);
    }
}

impl Drop for SleepWaker {
    fn drop(&mut self) {
        let Some(task) = self.task.take() else {
            return;
        };

        hyperion_log::error!("the waker wasn't woken before dropping it");
        READY.push(task);
    }
}

//

pub fn push(deadline: Instant, task: Task) {
    let mut fut = hyperion_events::timer::sleep_until(deadline);

    // poll the future with a custom waker
    let task = TakeOnce::new(task);
    let waker = waker(Arc::new(SleepWaker {
        task,
        #[cfg(debug_assertions)]
        deadline,
    }));
    let mut cx = Context::from_waker(&waker);
    if fut.poll_unpin(&mut cx).is_ready() {
        cx.waker().wake_by_ref();
    }
}
