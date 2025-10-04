# Lab 1: Scheduling

---

## Information

Name: 何梓源

Email: 2300012806@stu.pku.edu.cn

> Please cite any forms of information source that you have consulted during finishing your assignment, except the TacOS documentation, course slides, and course staff.

> With any comments that may help TAs to evaluate your work better, please leave them here

## Alarm Clock

### Data Structures

> A1: Copy here the **declaration** of every new or modified struct, enum type, and global variable. State the purpose of each within 30 words.

```rust
pub struct SleepData {
    pub ticks: i64,
    pub thread: Arc<Thread>,
}
// this data structure stores pairs, indicating <thread> should be waken up when <tick> is reached.
type SleepQueue = Mutex<alloc::collections::BinaryHeap<SleepData>>;
// this data structure offers an efficient and convenient approach to find the thread that wakes up earliest and insert new pairs.
pub static SLEEP_QUEUE: Lazy<SleepQueue> = Lazy::new(|| {
    let ret: mutex::Mutex<alloc::collections::binary_heap::BinaryHeap<SleepData>, intr::Intr> =
        Mutex::new(alloc::collections::BinaryHeap::new());
    ret
});
// as rust only accepts static global var and cannot be initialized with new() functuion, which causes the object no determined size.
// So, lazy is needed to delay initialization.
```


### Algorithms

> A2: Briefly describe what happens in `sleep()` and the timer interrupt handler.

In sleep(), parameter is checked. if time <= zero, return immediately.
Otherwise, register the desired wake up time in SleepQueue, then block current process.

> A3: What are your efforts to minimize the amount of time spent in the timer interrupt handler?

Firstly, I store wake up time information instead of sleep duration so that only a simple comparison is needed to identify whether a thread should be waken up.   
Secondly, I used a binary heap to store wake up time information because we only need to access the information about the thread which wakes up earliest at any time.


### Synchronization

> A4: How are race conditions avoided when `sleep()` is being called concurrently?

The global data sleep queue is protected by a mutex lock to avoid race conditions.

> A5: How are race conditions avoided when a timer interrupt occurs during a call to `sleep()`?

interupts are closed during sleep function and restored later.

## Priority Scheduling

### Data Structures

> B1: Copy here the **declaration** of every new or modified struct, enum type, and global variable. State the purpose of each within 30 words.

```rust
pub struct PriorityScheduler([VecDeque<Arc<Thread>>; 64]);
// this represents a seperated link list to store differnt threads of different priority in a seperated way.
pub struct DonationData {
    pub donner: Arc<Thread>,
    pub acceptor: Arc<Thread>,
    pub donner_priority: u32,
    pub prev_priority: u32,
    pub is_donner: bool,
    pub lockid: u32,
}
// this data struction is putted in every thread involved in a donation process to record the information of a donation.
static GLOBAL_LOCKID: AtomicU32 = AtomicU32::new(0);
// this global data is used in donation_data.lockid, making it possible to generate an unique number for all locks.


// in thread...
    pub donationq: Mutex<VecDeque<DonationData>>,
//  DonationData is stored in thread data.
    pub stored_prev: Mutex<(u32, u32)>,
// stored_prev stores a tuple of lockid-restored priority, in case that a locke is released but its previous priority cannot be restored because it is shaddowed by another donation. 
```

> B2: Explain the data structure that tracks priority donation. Clarify your answer with any forms of diagram (e.g., the ASCII art).

```txt
|----------------------------------|    
|           DonationData           |           
|    pub donner: Arc<Thread>,      |    
|    pub acceptor: Arc<Thread>,    |    
|    pub donner_priority: u32,     | <>----------------- Thread --------------------- <> Prev_data : <lockid,priority>    
|    pub prev_priority: u32,       |                                                              ^    
|    pub is_donner: bool,          |                                                              |    
|    pub lockid: u32,              |                                                              |    
|----------------------------------|----------------------when released---------------------------|    

```

### Algorithms

> B3: How do you ensure that the highest priority thread waiting for a lock, semaphore, or condition variable wakes up first?

I used seperated link-list to manage threads of different priority levels, and traverse from top when choosing new thread to run.

> B4: Describe the sequence of events when a thread tries to acquire a lock. How is nested donation handled?

If the lock is not holded by any other thread, P inner semaphore and register the current thread as holder.
Otherwise trying to donatate the current priority, which
(a) Judge if the potential donnor is of higher priority.
(b) If (a), donate the priorty
(c) If (a), record the donation in both donnor(used in nested donation) & acceptor thread(used to clean the donnor information more conveniently and find prev_priority when lock is released)
(d) If (a), find all donations in which at the current thread is donnor and recursively call the current function.
As demostrated above, in (d) the find_and_donate() function first calls wrapped_donation() then itself, using DFS to handle nested donations.

> B5: Describe the sequence of events when a lock, which a higher-priority thread is waiting for, is released.

Mainly two parts are needed
Part I: Identifying the correct previous priority
(a) Get the prev_priority field in record
(b) Record the data in a in Prev_data : (lockid,priority) which store information of released lock.
(c) If an older lock is released overwrite Prev_data.
(d) Check if there is any donations alive, if so, the previous priority should be shaddowed by the latest donation.
Part II: Clearing related donation data between holder <-> donner (may include many locks).



### Synchronization

> B6: Describe a potential race in `thread::set_priority()` and explain how your implementation avoids it. Can you use a lock to avoid this race?

set_priority() may race with Sleep::restore() to access thread priority data. I used an additional field priority_setted() to store setted priority. Lock cannot be used because set_priority() should have higher priority.

## Rationale

> C1: Have you considered other design possibilities? You can talk about anything in your solution that you once thought about doing them another way. And for what reasons that you made your choice?

I considered to implement the code to check which thread should be waken up in the schedule function instead of of in the time intr handler But this can caused trouble when initializing using schedule() to get into the INIT kernel thread and extra code have to be added to identify this case which makes the code more complicated.

I also considered using a hashmap to store thread-wake up time so that adding and deleting can be completed in O(1) time instead of O(log n) in binary heaps, but log (thread_num) seemed to be tolerable as thread_num is usually limited and using a min-heap intead of hashmap make it easier to estimate the time consumption when a more precise timming is needed.( When a hashmap collisioned or needed to be enlarged it might consume significantly longer time, which is unpredictable. )

I tried to make update() function, which adjust the seperated to be called only when set_priority() called instead of being called whenever schedule is called. This involves identifying (in run time?) "global variable manger -> scheduler" is of "priority scheduler type / implement priority trait" and I failed to work out the correct Rust gramma to do this.
