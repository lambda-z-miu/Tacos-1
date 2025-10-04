use core::{cmp::Reverse, sync::atomic::Ordering};

use alloc::collections::VecDeque;
use alloc::sync::Arc;
use fdt::standard_nodes::Chosen;

use crate::thread::{current, scheduler::priority, Schedule, Status, Thread};
use core::array;

/// Priority scheduler.
// #[derive(Default)]
pub struct PriorityScheduler([VecDeque<Arc<Thread>>; 64]);

impl Default for PriorityScheduler {
    fn default() -> Self {
        PriorityScheduler(array::from_fn(|_| VecDeque::new()))
    }
}

impl Schedule for PriorityScheduler {
    fn register(&mut self, thread: Arc<Thread>) {
        /*kprintln!(
            "registed thread, id {}, name {}, at priority {}",
            thread.id(),
            thread.name(),
            thread.priority.load(Ordering::SeqCst)
        );*/
        self.0[thread.priority.load(Ordering::SeqCst) as usize].push_front(thread);
    }

    fn schedule(&mut self) -> Option<Arc<Thread>> {
        self.update_priority();
        let mut called: bool = false;
        for i in (0..64) {
            if !(self.0)[i].is_empty() {
                for j in &self.0[i] {
                    // kprint!("({} at {}) ", j.id(), i);
                }
                called = true;
            }
        }
        if (called) {
            // kprintln!(" <- available");
        }

        // let current_priority = current().priority.load(Ordering::SeqCst);
        // let mut chosen: Option<Arc<Thread>> = None;
        // let mut chosen_priotiry = 0;

        for i in (0..64).rev() {
            if !(self.0)[i].is_empty() {
                let chosen = self.0[i].pop_back().unwrap();
                // chosen_priotiry = chosen.clone().unwrap().priority.load(Ordering::SeqCst);

                if chosen.priority.load(Ordering::SeqCst)
                    < current().priority.load(Ordering::SeqCst)
                    && current().status() == Status::Running
                {
                    self.0[i].push_back(chosen);
                    return None;
                } else {
                    /*
                    kprintln!(
                        "chosen thread, id {}, name {}, at priority {}",
                        chosen.id(),
                        chosen.name(),
                        chosen.priority.load(Ordering::SeqCst),
                    );*/
                    return Some(chosen);
                }
            }
        }
        return None;
    }
}

impl PriorityScheduler {
    fn update_priority(&mut self) {
        for i in 0..64 {
            let mut temp = VecDeque::new();
            while !self.0[i].is_empty() {
                let mut newitem = self.0[i].pop_front().unwrap();
                if newitem.priority.load(Ordering::SeqCst) == i as u32 {
                    temp.push_back(newitem);
                } else {
                    self.0[newitem.priority.load(Ordering::SeqCst) as usize].push_back(newitem);
                }
            }
            self.0[i] = temp;
        }
    }
}
