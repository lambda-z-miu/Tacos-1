use alloc::collections::VecDeque;
use alloc::sync::Arc;
use core::cell::{Cell, RefCell};

use crate::sbi;
use crate::thread::{self, current, Manager, Status, Thread};
use core::sync::atomic::Ordering;

/// Atomic counting semaphore
///
/// # Examples
/// ```
/// let sema = Semaphore::new(0);
/// sema.down();
/// sema.up();
/// ```
#[derive(Clone)]
pub struct Semaphore {
    value: Cell<usize>,
    waiters: RefCell<VecDeque<Arc<Thread>>>,
}

unsafe impl Sync for Semaphore {}
unsafe impl Send for Semaphore {}

impl Semaphore {
    /// Creates a new semaphore of initial value n.
    pub const fn new(n: usize) -> Self {
        Semaphore {
            value: Cell::new(n),
            waiters: RefCell::new(VecDeque::new()),
        }
    }

    /// P operation
    pub fn down(&self) {
        let old = sbi::interrupt::set(false);

        // Is semaphore available?
        while self.value() == 0 {
            // `push_front` ensures to wake up threads in a fifo manner
            self.waiters.borrow_mut().push_front(thread::current());

            // Block the current thread until it's awakened by an `up` operation
            // kprintln!("downcalled {}", self.value.get());
            thread::block();
        }
        self.value.set(self.value() - 1);
        // kprintln!("down complete, value is {}", self.value.get());

        sbi::interrupt::set(old);
    }

    /// V operation
    pub fn up(&self) {
        let old = sbi::interrupt::set(false);
        let count = self.value.replace(self.value() + 1);

        let waitqlen: usize = self.waiters.borrow().len();
        // kprintln!("waitqlen is {} ,value is {}", waitqlen, self.value.get());
        if waitqlen != 0 {
            kprintln!(
                "tid {} named {} at {} priority is waiting",
                self.waiters.borrow()[0].name(),
                self.waiters.borrow()[0].id(),
                self.waiters.borrow()[0].priority.load(Ordering::SeqCst)
            );
            assert_eq!(count, 0);
            assert_ne!(self.value(), 0);
            let (max_index, max_priority) = self.get_maxpriority();
            let thread = self.waiters.borrow_mut().remove(max_index);
            thread::wake_up(thread.expect("error finding index"));
            if current().priority.load(Ordering::SeqCst) < max_priority {
                kprintln!("max index {} qlen {}", max_index, waitqlen);
                // self.waiters.borrow()[max_index].status() != Status::Blocked;

                kprintln!(
                    "scheduled out current thread tid {} name {} at {} priority",
                    current().id(),
                    current().name(),
                    current().priority.load(Ordering::SeqCst)
                );
                sbi::interrupt::set(old);
                Manager::get().schedule();
            }
        }

        // Check if we need to wake up a sleeping waiter

        /*
        if let Some(thread) = self.waiters.borrow_mut().pop_back() {
            assert_eq!(count, 0);

            thread::wake_up(thread.clone());
        } */
        sbi::interrupt::set(old);
    }

    pub fn get_maxpriority(&self) -> (usize, u32) {
        let waitqlen: usize = self.waiters.borrow().len();
        let mut max_idex = 0;
        let mut max_priority = 0;
        for i in 0..waitqlen {
            let this_priority = self.waiters.borrow()[i].priority.load(Ordering::SeqCst);
            if this_priority >= max_priority {
                max_priority = this_priority;
                max_idex = i;
            }
        }
        return (max_idex, max_priority);
    }

    /// Get the current value of a semaphore
    pub fn value(&self) -> usize {
        self.value.get()
    }
}
