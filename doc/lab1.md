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

in sleep(), parameter is checked. if time <= zero, return immediately.
otherwise, register the desired wake up time in SleepQueue, then mark current process as blocked.

> A3: What are your efforts to minimize the amount of time spent in the timer interrupt handler?


### Synchronization

> A4: How are race conditions avoided when `sleep()` is being called concurrently?
close intr and

> A5: How are race conditions avoided when a timer interrupt occurs during a call to `sleep()`?

## Priority Scheduling

### Data Structures

> B1: Copy here the **declaration** of every new or modified struct, enum type, and global variable. State the purpose of each within 30 words.

> B2: Explain the data structure that tracks priority donation. Clarify your answer with any forms of diagram (e.g., the ASCII art).

### Algorithms

> B3: How do you ensure that the highest priority thread waiting for a lock, semaphore, or condition variable wakes up first?

> B4: Describe the sequence of events when a thread tries to acquire a lock. How is nested donation handled?

> B5: Describe the sequence of events when a lock, which a higher-priority thread is waiting for, is released.

### Synchronization

> B6: Describe a potential race in `thread::set_priority()` and explain how your implementation avoids it. Can you use a lock to avoid this race?

## Rationale

> C1: Have you considered other design possibilities? You can talk about anything in your solution that you once thought about doing them another way. And for what reasons that you made your choice?

Why a lock will be double acquired if we put the logic of checking whether a thread should be waken up in schedule() function, instead of updating it every tick, which appears to have a larger overhead.

In.
