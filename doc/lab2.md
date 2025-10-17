# Lab 2: User Programs

---

## Information

Name: heziyuan

Email: 2300012806@stu.pku.edu.cn

> Please cite any forms of information source that you have consulted during finishing your assignment, except the TacOS documentation, course slides, and course staff.

I got the advise 
1) to implement system call to pass args-none case 
2) to start from a new branch   
From Yuyang Hu 22, EECS.

> With any comments that may help TAs to evaluate your work better, please leave them here

I summarized some bugs I met and mistakes I made in lab-1 in lab1-Myguide.

## Argument Passing

#### DATA STRUCTURES

> A1: Copy here the **declaration** of each new or changed struct, enum type, and global variable. State the purpose of each within 30 words.
```rust
pub struct Thread {
...
    pub children: Mutex<Vec<Arc<Thread>>>,
    pub completed: Semaphore,
    pub fd: Mutex<BTreeMap<u32, (File, FdFlags)>>,
}
// added field children, completed to record child information
// added fd to record associated file descriptors

pub struct FdFlags {
    pub flag: usize,
}
// added Flags to manage open flag
```


#### ALGORITHMS

> A2: Briefly describe how you implemented argument parsing. How do you arrange for the elements of argv[] to be in the right order? How do you avoid overflowing the stack page?

1) Page table is cloned from kernel and activated. 
2) Elf is loaded and segments are mapped and returned the entry point and stack position.
3) The system handler frame is constructed to record info from 2.
4) Argument len is checked to be in a page. If so, push argv one by one.
5) argc is counted allocated on stack, putting the first argument on stack top.
6) Thread information is added to construct new thread running function start which do register level works to switch context. 

```rust
    let mut tot_len = argv.len() * 8 + 8;
    for t in argv {
        tot_len += t.len();
    }
    if tot_len > 4096 {
        panic!("TOO LONG ARGUMENT");
    }
```
Such check is employed to avoid overflowing stack page.

#### RATIONALE

> A3: In Tacos, the kernel reads the executable name and arguments from the command. In Unix-like systems, the shell does this work. Identify at least two advantages of the Unix approach.

- Implementing argument parsing in bash instead of kernel is a better to protect OS kernel. As user input is not guaranteed, if the kernel code does not perform adequate check for argument parsing, the OS kernel may fail.
- Implementing argument parsing in bash instead of kernel also provides better extensibility. When introducing more argument passing patterns, such as default parameter value, the kernel needs not to be changed.


## System Calls

#### DATA STRUCTURES

> B1: Copy here the **declaration** of each new or changed struct, enum type, and global variable. State the purpose of each within 30 words.

```rust
pub fd: Mutex<BTreeMap<u32, (File, FdFlags)>>,
// use a BTreeMap to record the mapping from a file descriptor to an opened file and an open flag in every thread.
```

> B2: Describe how file descriptors are associated with open files. Are file descriptors unique within the entire OS or just within a single process?

file descriptor refers to an opened map through fd map in every thread. so the fd in unique only in single process.

#### ALGORITHMS

> B3: Describe your code for reading and writing user data from the kernel.

Firstly special files like stdin, stdout, and stderror is handled independently. Then buf ptr validity, fd validity, R/W permission is checked. If check failed, return -1. then read in File trait is called, if returned error, also return -1, otherwise the value in Ok.

> B4: Suppose a system call causes a full page (4,096 bytes) of data to be copied from user space into the kernel. What is the least and the greatest possible number of inspections of the page table (e.g. calls to `Pagetable.get_pte(addr)` or other helper functions) that might result? What about for a system call that only copies 2 bytes of data? Is there room for improvement in these numbers, and how much?

2;2. For every pointer, the first two byte has to be checked, and for the rest, in every successive 4096 bytes, one byte has to be checked. This optimization is safe because any page fault will create a hole at least 4096 bytes long on address space.

> B5: Briefly describe your implementation of the "wait" system call and how it interacts with process termination.

Every thread maintains a completed filed. It is a semaphore with initial value 0, when completed, up() is called. In wait() completed.down() is called to wait for the thread to call exit() system call.

> B6: Any access to user program memory at a user-specified address can fail due to a bad pointer value.  Such accesses must cause th process to be terminated.  System calls are fraught with such accesses, e.g. a "write" system call requires reading the system call number from the user stack, then each of the call's three arguments, then an arbitrary amount of user memory, and any of these can fail at any point.  This poses a design and error-handling problem: how do you best avoid obscuring the primary function of code in a morass of error-handling?  Furthermore, when an error is detected, how do you ensure that all temporarily allocated resources (locks, buffers, etc.) are freed? Have you used some features in Rust, to make these things easier than in C? In a few paragraphs, describe the strategy or strategies you adopted for managing these issues.  Give an example.

!!! I used C style error handling. It can be clearly managed if we only focus on cases that returns normal value, and the rest is -1. 

> B7: Briefly describe what will happen if loading the new executable fails. (e.g. the file does not exist, is in the wrong format, or some other error.)

1) In execute_handler, arguments are checked to avoid dereferencing illegal pointers.
2) When Open returned -1, the mistake is propagated to return -1 in execute_handler.
3) When ELF is not of correct format, load_elf returns error and execute() returns -1.

#### SYNCHRONIZATION

> B8: Consider parent process P with child process C.  How do you ensure proper synchronization and avoid race conditions when P calls wait(C) before C exits?  After C exits?  How do you ensure that all resources are freed in each case?  How about when P terminates without waiting, before C exits?  After C exits?  Are there any special cases?

As mentioned in B5. Every thread maintains a completed filed. It is a semaphore with initial value 0, when completed, up() is called. In wait() completed.down() is called to wait for the thread to call exit() system call. For resource safety, when exit() is called, the thread destroys its page table such releasing all memory used by text, stack or heap. Also, clean_fd() is called to traverse fd map and tries to close them one by one.



#### RATIONALE

> B9: Why did you choose to implement access to user memory from the kernel in the way that you did?

I store the page table in the thread struct so that when entered kernel state, the user memory mapping is still valid. Also, user space memories has to be checked before accessed to prevent page fault in kernel.

> B10: What advantages or disadvantages can you see to your design for file descriptors?

Unique file descriptors in each process makes it simpler and safer to manage fd. As for simpler, opening, reading, writing can be done in process level with no global side effect. As for safer, the process cannot access information in the files opened by other process, which eliminated privacy leaks. 

> B11: What is your tid_t to pid_t mapping. What advantages or disadvantages can you see to your design?

I used a uniformed way to handle them, it is convenient but less efficient.
