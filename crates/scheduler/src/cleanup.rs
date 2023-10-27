use hyperion_instant::Instant;

use crate::{ipc, schedule, sleep, task::Task};

//

pub struct CleanupTask {
    task: Task,
    cleanup: Cleanup,
}

impl CleanupTask {
    pub fn run(self) {
        self.cleanup.run(self.task);
    }
}

//

#[derive(Debug, Clone, Copy)]
pub enum Cleanup {
    Sleep { deadline: Instant },
    SimpleIpcWait,
    Drop,
    Ready,
}

impl Cleanup {
    pub const fn task(self, task: Task) -> CleanupTask {
        CleanupTask {
            task,
            cleanup: self,
        }
    }

    pub fn run(self, task: Task) {
        match self {
            Self::Sleep { deadline } => sleep::push(deadline, task),
            Self::SimpleIpcWait => ipc::start_waiting(task),
            Self::Drop => {}
            Self::Ready => {
                schedule(task);
            }
        }
    }
}
