use alloc::sync::Arc;
use core::cell::RefCell;
use core::sync::atomic::{AtomicU32, Ordering};

use crate::sync::{Lock, Semaphore};
use crate::thread::{self, current, manager, Manager, Thread};

/// Sleep lock. Uses [`Semaphore`] under the hood.
#[derive(Clone)]
pub struct Sleep {
    inner: Semaphore,
    holder: RefCell<Option<Arc<Thread>>>,
    prev_priority: RefCell<Option<Arc<AtomicU32>>>,
}

impl Default for Sleep {
    fn default() -> Self {
        Self {
            inner: Semaphore::new(1),
            holder: Default::default(),
            prev_priority: RefCell::new(None),
        }
    }
}

impl Lock for Sleep {
    fn acquire(&self) {
        if self.holder.borrow().is_some() {
            kprintln!("CALLED:LOCK");
            let current_holder: Arc<Thread> = self.holder.borrow().as_ref().unwrap().clone();

            let current_priority: u32 = current().priority.load(Ordering::SeqCst);
            let holder_priority: u32 = current_holder.priority.load(Ordering::SeqCst);
            if current_priority > holder_priority {
                //
                kprintln!("cur prio {},hld prio {}", current_priority, holder_priority);

                // store prev_priority
                let temp = Arc::new(AtomicU32::new(holder_priority));
                self.prev_priority.borrow_mut().replace(temp);

                // donate current_priority
                current_holder
                    .priority
                    .store(current_priority, Ordering::SeqCst);
                kprintln!(
                    "after donation, thread which is waited has {} priority where it used to have {}",
                    current_priority,
                    holder_priority
                );
                Manager::get().schedule();
            }
        }
        self.inner.down();
        self.holder.borrow_mut().replace(thread::current());
    }

    fn release(&self) {
        assert!(Arc::ptr_eq(
            self.holder.borrow().as_ref().unwrap(),
            &thread::current()
        ));

        if self.prev_priority.borrow().is_some() {
            // restore donation
            let prev_prio_value = self
                .prev_priority
                .borrow()
                .as_ref()
                .unwrap()
                .load(Ordering::SeqCst);
            kprintln!(
                "priority restored to {}, from {}",
                prev_prio_value,
                current().priority.load(Ordering::SeqCst)
            );
            current().priority.store(prev_prio_value, Ordering::SeqCst);

            // clear prev_priority
            *self.prev_priority.borrow_mut() = None;
        }

        if current().priority_setted.lock().is_some() {
            //use setted priority to overwite restored ones
            let setted = current().priority_setted.lock().unwrap();
            current().priority.store(setted, Ordering::SeqCst);
        }

        self.holder.borrow_mut().take().unwrap();
        self.inner.up();
        Manager::get().schedule();
    }
}

unsafe impl Sync for Sleep {}
