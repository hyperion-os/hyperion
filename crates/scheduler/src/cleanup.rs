use hyperion_instant::Instant;

use crate::{schedule, sleep, task::Task, READY};

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
            Self::Sleep { deadline } => {
                sleep::push(deadline, task);
                for ready in sleep::finished() {
                    READY.push(ready);
                }
            }
            Self::SimpleIpcWait => {
                let proc = task.process.clone();

                if !proc.simple_ipc.channel.is_empty() {
                    READY.push(task);
                } else {
                    proc.simple_ipc.waiting.push(task);
                }
            }
            Self::Drop => {}
            Self::Ready => {
                schedule(task);
            }
        }
    }
}
