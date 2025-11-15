# Lab 3: Virtual Memory

---

## Information

Name:

Email:

> Please cite any forms of information source that you have consulted during finishing your assignment, except the TacOS documentation, course slides, and course staff.

> With any comments that may help TAs to evaluate your work better, please leave them here.
>
> My page eviction algorithm (`select_page`) is a global, timestamp-based approximate LRU, which does not rely on PTE Accessed/Dirty bits. For synchronization, I avoid expensive locks bundling `select_page` and `swapout`. Instead, I use an optimistic "livelock" retry mechanism to handle race conditions between page faults and process exits.

## Stack Growth

#### ALGORITHMS

> A1: Explain your heuristic for deciding whether a page fault for an invalid virtual address should cause the stack to be extended into the page that faulted.

When a page fault occurs at an invalid virtual address, my handler determines it is a valid stack growth request if all of the following conditions are met:
1. The faulting address is below the current user stack pointer (`sp`).
2. The address is within a small threshold of `sp` (e.g., 32 bytes) to accommodate instructions like `PUSH`.
3. The address is within a reasonable user-space range and not too low (e.g., above `0x100000`) to avoid conflicts with the null pointer area or kernel space.

If an address satisfies these conditions, I consider it a legitimate stack growth request. I then allocate a new physical frame, zero it out, and map it to the faulting virtual address, effectively extending the stack downwards by one page.

## Memory Mapped Files

#### DATA STRUCTURES

> B1: Copy here the declaration of each new or changed struct or struct member, global or static variable, typedef, or enumeration. Identify the purpose of each in 25 words or less.

(This feature is not implemented in my code.)

#### ALGORITHMS

> B2: Describe how memory mapped files integrate into your virtual memory subsystem. Explain how the page fault and eviction processes differ between swap pages and other pages.

(This feature is not implemented in my code.)

> B3: Explain how you determine whether a new file mapping overlaps any existing segment.

(This feature is not implemented in my code.)

#### RATIONALE

> B4: Mappings created with "mmap" have similar semantics to those of data demand-paged from executables, except that "mmap" mappings are written back to their original files, not to swap. This implies that much of their implementation can be shared. Explain why your implementation either does or does not share much of the code for the two situations.

(This feature is not implemented in my code.)

## Page Table Management

#### DATA STRUCTURES

> C1: Copy here the **declaration** of each new or changed struct, enum type, and global variable. State the purpose of each within 30 words.

```rust
// In mem/swapmanager.rs
pub struct FrameTableEntry {
    pub stored_va: usize, // Virtual address corresponding to the frame.
    pub pa: usize,        // Physical address of the frame.
    pub tid: isize,      // Thread ID using the frame, -1 if free.
    pub flags: usize,    // Metadata flags, e.g., dirty or accessed.
    pub pte: *mut usize, // Pointer to the page table entry.
}
```

(This feature is not implemented in my code.)

#### ALGORITHMS

> C2: In a few paragraphs, describe your code for accessing the data stored in the Supplementary page table about a given page.

> C3: How does your code coordinate accessed and dirty bits between kernel and user virtual addresses that alias a single frame, or alternatively how do you avoid the issue?

#### SYNCHRONIZATION

> C4: When two user processes both need a new frame at the same time, how are races avoided?

#### RATIONALE

> C5: Why did you choose the data structure(s) that you did for representing virtual-to-physical mappings?

## Paging To And From Disk

#### DATA STRUCTURES

> D1: Copy here the **declaration** of each new or changed struct, enum type, and global variable. State the purpose of each within 30 words.

```rust
// In mem/swapmanager.rs
pub struct FrameTableEntry {
    pub stored_va: usize, // Virtual address corresponding to the frame.
    pub pa: usize,        // Physical address of the frame.
    pub tid: isize,      // Thread ID using the frame, -1 if free.
    pub flags: usize,    // Metadata flags, e.g., dirty or accessed.
    pub pte: *mut usize, // Pointer to the page table entry.
}
```

(This feature is not implemented in my code.)

#### ALGORITHMS

> D2: When a frame is required but none is free, some frame must be evicted. Describe your code for choosing a frame to evict.

> D3: When a process P obtains a frame that was previously used by a process Q, how do you adjust the page table (and any other data structures) to reflect the frame Q no longer has?

#### SYNCHRONIZATION

> D5: Explain the basics of your VM synchronization design. In particular, explain how it prevents deadlock. (Refer to the textbook for an explanation of the necessary conditions for deadlock.)

> D6: A page fault in process P can cause another process Q's frame to be evicted. How do you ensure that Q cannot access or modify the page during the eviction process? How do you avoid a race between P evicting Q's frame and Q faulting the page back in?

> D7: Suppose a page fault in process P causes a page to be read from the file system or swap. How do you ensure that a second process Q cannot interfere by e.g. attempting to evict the frame while it is still being read in?

> D8: Explain how you handle access to paged-out pages that occur during system calls. Do you use page faults to bring in pages (as in user programs), or do you have a mechanism for "locking" frames into physical memory, or do you use some other design? How do you gracefully handle attempted accesses to invalid virtual addresses?

#### RATIONALE

> D9: A single lock for the whole VM system would make synchronization easy, but limit parallelism. On the other hand, using many locks complicates synchronization and raises the possibility for deadlock but allows for high parallelism. Explain where your design falls along this continuum and why you chose to design it this way.
