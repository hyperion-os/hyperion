use anyhow::Result;
use hyperion_num_postfix::NumberPostfix;
use libstd::{println, sys::timestamp};

//

pub fn cmd<'a>(_: impl Iterator<Item = &'a str>) -> Result<()> {
    let uptime = time::Duration::nanoseconds(timestamp().unwrap() as _);

    let uptime_h = uptime.whole_hours();
    let uptime_m = uptime.whole_minutes() % 60;
    let uptime_s = uptime.whole_seconds() % 60;

    /* let tasks = hyperion_scheduler::tasks();
    let task_states = tasks.iter().map(|task| task.state.load()); */
    // TODO:
    // let tasks_running = TASKS_RUNNING.load(Ordering::Relaxed);
    // let tasks_sleeping = TASKS_SLEEPING.load(Ordering::Relaxed);
    // let tasks_ready = TASKS_READY.load(Ordering::Relaxed);
    // let tasks_total = tasks_running + tasks_sleeping + tasks_ready;

    let (total, free, used) = super::mem::read_meminfo()?;
    let total = total.postfix_binary();
    let free = free.postfix_binary();
    let used = used.postfix_binary();

    println!("top - {uptime_h}:{uptime_m:02}:{uptime_s:02} up");
    // _ = writeln!(
    //         self.term,
    //         "Tasks: {tasks_total} total, {tasks_running} running, {tasks_sleeping} sleeping, {tasks_ready} ready"
    //     );
    println!("Mem: {total}B total, {free}B free, {used}B used");

    // TODO:
    // println!("Cpu idles: ");
    // for idle in idle() {
    //     // round the time
    //     let idle = time::Duration::milliseconds(idle.whole_milliseconds() as _);
    //     print!("{idle}, ");
    // }
    // println!();

    super::ps::cmd([].into_iter())?;

    Ok(())
}
