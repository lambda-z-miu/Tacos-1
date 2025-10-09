use alloc::collections::vec_deque::VecDeque;
use alloc::sync::Arc;
use core::cell::RefCell;
use core::sync::atomic::{AtomicU32, Ordering};

use crate::sync::{Lock, Semaphore};
use crate::thread::{self, current, manager, Manager, Thread};
use core::cmp::max;

// Global LockID
static GLOBAL_LOCKID: AtomicU32 = AtomicU32::new(0);

#[derive(Clone)]
pub struct DonationData {
    pub donner: Arc<Thread>,
    pub acceptor: Arc<Thread>,
    pub donner_priority: u32,
    pub prev_priority: u32,
    pub is_donner: bool,
    pub lockid: u32,
}

/// Sleep lock. Uses [`Semaphore`] under the hood.
#[derive(Clone)]
pub struct Sleep {
    lockid: u32,
    inner: Semaphore,
    holder: RefCell<Option<Arc<Thread>>>,
    prev_priority: RefCell<Option<Arc<AtomicU32>>>,
}

impl Default for Sleep {
    fn default() -> Self {
        GLOBAL_LOCKID.fetch_add(1, Ordering::SeqCst);
        Self {
            lockid: GLOBAL_LOCKID.load(Ordering::SeqCst),
            inner: Semaphore::new(1),
            holder: Default::default(),
            prev_priority: RefCell::new(None),
        }
    }
}

pub fn donation_wrapped(donner: Arc<Thread>, acceptor: Arc<Thread>, lockid: u32) {
    let current_priority: u32 = donner.priority.load(Ordering::SeqCst);
    let holder_priority: u32 = acceptor.priority.load(Ordering::SeqCst);

    // if donation is needed
    if current_priority > holder_priority {
        // call donate function
        donner.donate(acceptor.clone());

        // construct DonationData
        let current_dd = DonationData {
            donner: donner.clone(),
            acceptor: acceptor.clone(),
            donner_priority: current_priority,
            prev_priority: holder_priority,
            is_donner: true,
            lockid: lockid,
        };
        donner.add_donation(current_dd.clone());
        let mut acceptor_dd = current_dd.clone();
        acceptor_dd.is_donner = false;
        acceptor.add_donation(acceptor_dd);
        // register the donation relationship in current & acceptor.

        kprintln!("cur prio {},hld prio {}", current_priority, holder_priority);
        Manager::get().schedule();
    }
}

impl Lock for Sleep {
    fn acquire(&self) {
        if self.holder.borrow().is_some() {
            let mut acceptor: Arc<Thread> = self.holder.borrow_mut().as_mut().unwrap().clone();

            donation_wrapped(current(), acceptor.clone(), self.lockid);
            crate::thread::Thread::find_and_donate(acceptor, self.lockid);
        }
        self.inner.down();
        // kprintln!("lock {} is acquired by {}", self.lockid, current().id());
        self.holder.borrow_mut().replace(thread::current());
    }

    /// called in acceptor thread
    fn release(&self) {
        // kprintln!("lockid {} is released", self.lockid);
        assert!(Arc::ptr_eq(
            self.holder.borrow().as_ref().unwrap(),
            &thread::current()
        ));

        let ret = current().delete_donation(self.lockid);
        if let Some(ret) = ret {
            let donner = ret.donner;
            donner.delete_donation(self.lockid);

            current()
                .donationq
                .lock()
                .retain(|x| x.donner.id() != donner.id());
            donner
                .donationq
                .lock()
                .retain(|x| x.acceptor.id() != current().id());

            let prev = ret.prev_priority;
            if current().stored_prev.lock().0 > self.lockid {
                current().stored_prev.lock().0 = self.lockid;
                current().stored_prev.lock().1 = prev;
            }

            let mut final_priority = current().stored_prev.lock().1;

            for i in current().donationq.lock().clone().into_iter() {
                kprintln!("!!!!!!{}{}{}", i.donner.id(), i.acceptor.id(), i.lockid);
                final_priority = max(final_priority, i.donner_priority);
            }

            kprintln!(
                "thread id {} , priotiry reset to {}",
                current().id(),
                current().priority.load(Ordering::SeqCst)
            );
            current().priority.store(final_priority, Ordering::SeqCst);
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
