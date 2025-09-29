use crate::{
    sbi::{self, timer::tick},
    sync::{intr, mutex, Lazy, Lock},
};
use alloc::sync::Arc;
use thread::*;
pub struct SleepData {
    pub ticks: i64,
    pub thread: Arc<Thread>,
}

impl PartialOrd for SleepData {
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        Some(self.cmp(other))
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

type SleepQueue = Mutex<alloc::collections::BinaryHeap<SleepData>>;

pub static SLEEP_QUEUE: Lazy<SleepQueue> = Lazy::new(|| {
    let ret: mutex::Mutex<alloc::collections::binary_heap::BinaryHeap<SleepData>, intr::Intr> =
        Mutex::new(alloc::collections::BinaryHeap::new());
    // kprintln!("SLEEP_Q INIT");
    ret
});

impl SleepQueue {
    pub fn len(&self) -> usize {
        self.lock().len()
    }
    pub fn pop(&self) -> SleepData {
        self.lock().pop().unwrap()
    }
    pub fn push(&self, data: SleepData) {
        self.lock().push(data);
    }
    pub fn peek_time(&self) -> i64 {
        self.lock().peek().unwrap().ticks.clone()
    }
}
