//! # Condition Variable
//!
//! [`Condvar`] are able to block a thread so that it consumes no CPU time
//! while waiting for an event to occur. It is typically associated with a
//! boolean predicate (a condition) and a mutex. The predicate is always verified
//! inside of the mutex before determining that a thread must block.
//!
//! ## Usage
//!
//! Suppose there are two threads A and B, and thread A is waiting for some events
//! in thread B to happen.
//!
//! Here is the common practice of thread A:
//! ```rust
//! let pair = Arc::new(Mutex::new(false), Condvar::new());
//!
//! let (lock, cvar) = &*pair;
//! let condition = lock.lock();
//! while !condition {
//!     cvar.wait(&condition);
//! }
//! ```
//!
//! Here is a good practice of thread B:
//! ```rust
//! let (lock, cvar) = &*pair;
//!
//! // Lock must be held during a call to `Condvar.notify_one()`. Therefore, `guard` has to bind
//! // to a local variable so that it won't be dropped too soon.
//!
//! let guard = lock.lock(); // Bind `guard` to a local variable
//! *guard = true;           // Condition change
//! cvar.notify_one();       // Notify (`guard` will overlive this line)
//! ```
//!
//! Here is a bad practice of thread B:
//! ```rust
//! let (lock, cvar) = &*pair;
//!
//! *lock.lock() = true;     // Lock won't be held after this line.
//! cvar.notify_one();       // Buggy: notify another thread without holding the Lock
//! ```
//!

use alloc::collections::VecDeque;
use alloc::sync::Arc;
use core::cell::RefCell;

use crate::{
    sync::{Lock, MutexGuard, Semaphore},
    thread::scheduler::priority,
};

pub struct Condvar(RefCell<VecDeque<Arc<Semaphore>>>);

unsafe impl Sync for Condvar {}
unsafe impl Send for Condvar {}

impl Condvar {
    pub fn new() -> Self {
        Condvar(Default::default())
    }

    pub fn wait<T, L: Lock>(&self, guard: &mut MutexGuard<'_, T, L>) {
        let sema = Arc::new(Semaphore::new(0));
        self.0.borrow_mut().push_front(sema.clone());

        guard.release();
        sema.down();
        guard.acquire();
    }

    /// Wake up one thread from the waiting list
    pub fn notify_one(&self) {
        let length = self.0.borrow().len();
        let mut chosen_index = 0;
        if !self.0.borrow().is_empty() {
            let mut max_priority = 0;
            for i in 0..length {
                let (_, this_priority) = self.0.borrow()[i].get_maxpriority();
                if this_priority >= max_priority {
                    max_priority = this_priority;
                    chosen_index = i;
                }
            }
        }

        let chosen = self.0.borrow_mut().remove(chosen_index);
        if let Some(sema) = chosen {
            sema.up();
        }
        /*
        if let Some(sema) = self.0.borrow_mut().pop_back() {
            sema.up();
        }*/
    }

    /// Wake up all waiting threads
    pub fn notify_all(&self) {
        self.0.borrow().iter().for_each(|s| s.up());
        self.0.borrow_mut().clear();
    }
}
