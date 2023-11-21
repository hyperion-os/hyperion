use core::{ptr::NonNull, sync::atomic::AtomicUsize};

use hyperion_instant::Instant;

use crate::{futex, schedule, sleep, task::Task};

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
    Sleep {
        deadline: Instant,
    },
    Wait {
        addr: NonNull<AtomicUsize>,
        val: usize,
    },
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
            Self::Wait { addr, val } => futex::cleanup(addr, val, task),
            Self::Drop => {}
            Self::Ready => {
                schedule(task);
            }
        }
    }
}

unsafe impl Send for Cleanup {}
