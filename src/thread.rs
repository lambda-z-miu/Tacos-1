//! Kernel Threads

mod imp;
pub mod manager;
pub mod scheduler;
pub mod switch;

use core::{cell::OnceCell, cmp::Reverse, ops::DerefMut};

use crate::{
    sbi::{self, timer::tick},
    sync::{intr, mutex, Lazy, Lock},
};

pub use self::imp::*;
pub use self::manager::Manager;
pub(self) use self::scheduler::{Schedule, Scheduler};

use alloc::sync::{self, Arc};
use riscv::interrupt;

pub struct SleepData {
    pub ticks: i64,
    pub thread: Arc<Thread>,
}

impl PartialOrd for SleepData {
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        Some(self.cmp(other).reverse())
    }
}

impl Ord for SleepData {
    fn cmp(&self, other: &Self) -> core::cmp::Ordering {
        other.ticks.cmp(&self.ticks)
    }
}

impl PartialEq for SleepData {
    fn eq(&self, other: &Self) -> bool {
        self.ticks == other.ticks
    }
}

impl Eq for SleepData {}

pub struct SleepQueue {
    // (wake_time, thread)
    queue: Lazy<Arc<Mutex<alloc::collections::BinaryHeap<SleepData>>>>,
}

static SLEEP_QUEUE: SleepQueue = SleepQueue {
    queue: Lazy::new(|| Arc::new(Mutex::new(alloc::collections::BinaryHeap::new()))),
};

/// Create a new thread
pub fn spawn<F>(name: &'static str, f: F) -> Arc<Thread>
where
    F: FnOnce() + Send + 'static,
{
    Builder::new(f).name(name).spawn()
}

/// Get the current running thread
pub fn current() -> Arc<Thread> {
    Manager::get().current.lock().clone()
}

/// Yield the control to another thread (if there's another one ready to run).
pub fn schedule() {
    // determine if a sleeping thread should be woken up

    /*
    use crate::sbi::timer::{timer_elapsed, timer_ticks};
    kprint!("ADDED_check_schedue {}", SLEEP_QUEUE.queue.lock().len());

    let now = timer_ticks();
    let mut queue = SLEEP_QUEUE.queue.lock();
    while let Some(queue_top) = queue.peek() {
        kprint!("TOP at {}", queue_top.ticks);
        if now >= queue_top.ticks {
            let ready_thread = queue.pop().unwrap().thread;
            SLEEP_QUEUE.queue.lock().deref_mut().pop();
            wake_up(ready_thread);
        } else {
            break;
        }
    } */

    Manager::get().schedule();
}

/// Gracefully shut down the current thread, and schedule another one.
pub fn exit() -> ! {
    {
        let current = Manager::get().current.lock();

        #[cfg(feature = "debug")]
        kprintln!("Exit: {:?}", *current);

        current.set_status(Status::Dying);
    }

    schedule();

    unreachable!("An exited thread shouldn't be scheduled again");
}

/// Mark the current thread as [`Blocked`](Status::Blocked) and
/// yield the control to another thread
pub fn block() {
    let current = current();
    current.set_status(Status::Blocked);

    #[cfg(feature = "debug")]
    kprintln!("[THREAD] Block {:?}", current);

    schedule();
}

/// Wake up a previously blocked thread, mark it as [`Ready`](Status::Ready),
/// and register it into the scheduler.
pub fn wake_up(thread: Arc<Thread>) {
    assert_eq!(thread.status(), Status::Blocked);
    thread.set_status(Status::Ready);

    #[cfg(feature = "debug")]
    kprintln!("[THREAD] Wake up {:?}", thread);

    Manager::get().scheduler.lock().register(thread);
}

/// (Lab1) Sets the current thread's priority to a given value
pub fn set_priority(_priority: u32) {}

/// (Lab1) Returns the current thread's effective priority.
pub fn get_priority() -> u32 {
    0
}

/// (Lab1) Make the current thread sleep for the given ticks.
pub fn sleep(ticks: i64) {
    use crate::sbi::timer::timer_ticks;
    use sbi::interrupt::set;

    // let old = crate::sync::Intr::set(false);

    // let time_locker = crate::sync::Intr::new();
    // Lock::acquire(&time_locker);
    kprintln!("CALLED SLEEP {}", timer_ticks());
    if ticks <= 0 {
        // Lock::release(&time_locker);
        return;
    }

    set(false);
    let curren_tick = timer_ticks();
    let current = current();
    let wake_time = curren_tick + ticks;
    SLEEP_QUEUE.queue.lock().push(
        (SleepData {
            ticks: wake_time,
            thread: current.clone(),
        }),
    );
    kprint!("CHECK ADDED {}", SLEEP_QUEUE.queue.lock().len());
    set(true);
    // Lock::release(&time_locker);
    block();

    /*
    while timer_elapsed(start) < ticks {
        schedule();
    }*/
}
