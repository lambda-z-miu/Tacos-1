# Lab 3: Virtual Memory

---

## Information

Name: heziyuan

Email: 2300012806@stu.pku.edu.cn

> Please cite any forms of information source that you have consulted during finishing your assignment, except the TacOS documentation, course slides, and course staff.

> With any comments that may help TAs to evaluate your work better, please leave them here.
>
> My page eviction algorithm (`select_page`) is a global, timestamp-based approximate LRU. For synchronization, I avoid expensive locks bundling `select_page` and `swapout`. Instead, I use an optimistic "livelock" retry mechanism to handle race conditions between page faults and process exits.

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

```rust
// In trap/memorytrap.rs
pub struct MmapData {
    pub mmap_id: u32,     // Unique ID for the mmap region.
    pub addr: usize,      // Start virtual address of the mapping.
    pub pages: usize,     // Number of pages in the mapping.
    pub fd: u32,          // File descriptor of the mapped file.
    pub len: u64,         // Exact length of the mapping in bytes.
    pub need_close: bool, // Flag indicating if the fd should be closed on unmap.
}

// In thread/mod.rs, added to Thread struct
pub mmap_info: Lock<Vec<MmapData>>, // Per-process list of all its memory mappings.
pub page_info: Lock<Vec<PageInfo>>, // Per-process list tracking the type of each allocated page.

// In mem/allocdata.rs
pub struct PageInfo {
    pub va: usize,          // The virtual address of the page.
    pub page_type: AllocType, // The type of the page (e.g., Stack, Heap, MemMap).
}
```

#### ALGORITHMS

> B2: Describe how memory mapped files integrate into your virtual memory subsystem. Explain how the page fault and eviction processes differ between swap pages and other pages.

**Integration:** When `mmap` is called, the `mmap_handler` validates the arguments (address alignment, file descriptor) and checks for overlapping virtual address ranges. It then creates a `MmapData` entry and records the `PageInfo` for the entire virtual address range, but does not allocate any physical pages. The pages are demand-paged upon first access. When a page fault occurs, the handler checks the `PageInfo` for the faulting address. If it's an `AllocType::MemMap`, it allocates a frame, reads the corresponding content from the file, and maps it.


> B3: Explain how you determine whether a new file mapping overlaps any existing segment.

My implementation checks for overlaps at the virtual address level. The `check_page_overlap` function acquires a lock on the current process's `page_info` list. It then iterates through this list, which contains records for all allocated virtual pages (stack, heap, and other mmaps), and checks if the starting virtual address of the new mapping (`va`) already exists. This prevents two different memory segments from being mapped to the same virtual address.
To speed up the process, the first, second address is checked then checker use a PG_SIZE stride to
check further items.

#### RATIONALE

> B4: Mappings created with "mmap" have similar semantics to those of data demand-paged from executables, except that "mmap" mappings are written back to their original files, not to swap. This implies that much of their implementation can be shared. Explain why your implementation either does or does not share much of the code for the two situations.

My implementation shares a significant amount of code. The core page fault handling logic is centralized. When a fault occurs, the handler first determines the page's type by consulting the `PageInfo` structure. The subsequent logic diverges based on this type:
*   **Shared Code:** The process of allocating a new physical frame, finding a victim page if necessary (`select_page`), and updating the page table structure is shared.
*   **Divergent Code:** The source of the data to populate the new frame is different. For an mmap page, data is read from the mapped file. For a new stack/heap page, the frame is zero-filled. For a swapped page, data is read from the swap file. Similarly, the write-back path is different: `unmap_handler` writes dirty mmap pages back to their files, while `swapout` writes other dirty pages to the swap file.

## Page Table Management

#### DATA STRUCTURES

> C1: Copy here the **declaration** of each new or changed struct, enum type, and global variable. State the purpose of each within 30 words.

```rust
// In mem/swapmanager.rs
pub struct FrameTableEntry {
    pub frame_va: usize,  // Kernel virtual address of the physical frame.
    pub stored_va: usize, // User virtual address mapped to this frame.
    pub tid: isize,       // ID of the thread owning this frame.
    pub time: isize,      // Timestamp for approximate LRU eviction policy.
    pub busy: bool,       // True if frame is being processed for eviction.
}

pub struct SwapTableEntry {
    pub addr: usize,        // The virtual page address.
    pub state: MemState,    // State of the page (InMem, Swapped, Exe).
    pub flags: PTEFlags,    // Original PTE flags to restore on swap-in.
    pub file_off: Option<u32>, // Offset in swap file or original file.
}

pub struct SwapManager {
    pub block_map: BTreeMap<u32, usize>, // Tracks used slots in the swap file.
    pub frame_table: VecDeque<FrameTableEntry>, // Global list of all active physical frames.
}

pub static TIMESTAMP: AtomicI64 = AtomicI64::new(0); // Global counter for setting frame timestamps.
```

#### ALGORITHMS

> C2: In a few paragraphs, describe your code for accessing the data stored in the Supplementary page table about a given page.

My Supplementary Page Table (named swap table in code) is a `VecDeque<SwapTableEntry>` named `swap_table` inside each `Thread` struct, protected by a `Lock`.

To access information about a given virtual page, my helper functions (`get_page_pos`, `get_page_flags`, `get_page_state`) first acquire the lock on the current thread's `swap_table`. They then perform a linear scan through the `VecDeque`, comparing the `addr` field of each `SwapTableEntry` with the target virtual address. Once the matching entry is found, the required data (e.g., `state`, `flags`, `file_off`) is returned. This simple, robust design avoids the complexity of a kernel-space hash map.

> C3: How does your code coordinate accessed and dirty bits between kernel and user virtual addresses that alias a single frame, or alternatively how do you avoid the issue?

My design completely avoids this issue because my eviction policy is independent of the hardware Accessed (A) and Dirty (D) bits. The `select_page` function chooses a victim based on a `time` timestamp stored in the global `frame_table`, implementing an approximate LRU policy.

While my `swapout_pt` function does save the full PTE flags (including A/D bits) to the SPT and `swapin` restores them, this is for correctness of page state, not for eviction decisions. Since the eviction algorithm never reads or relies on these bits, the problem of keeping them synchronized for aliased addresses is non-existent.

#### SYNCHRONIZATION

> C4: When two user processes both need a new frame at the same time, how are races avoided?

When two processes need a new frame, they both may call `select_page`. This function is protected by a global mutex `GLB_SWM`. This serializes the selection process. The first process to acquire the lock will scan the `frame_table`, choose a victim, and set its `busy` flag to `true` before releasing the lock. When the second process acquires the lock, it will see the `busy` flag and skip that frame, thus preventing both processes from choosing the same frame to evict.

#### RATIONALE

> C5: Why did you choose the data structure(s) that you did for representing virtual-to-physical mappings?

My design uses a multi-level approach for tracking mappings:
1.  **Hardware Page Table:** This is the standard RISC-V Sv39 structure, which is required by the hardware for address translation.
2.  **`VecDeque<SwapTableEntry>` (SPT):** I chose a `VecDeque` for the per-process supplementary page table due to its implementation simplicity. It reliably tracks the state of every page belonging to a process. While a hash map would be faster, a `VecDeque` is sufficient and avoids significant implementation complexity.
3.  **`VecDeque<FrameTableEntry>` (Global Frame Table):** A global, centralized list of all physical frames is necessary for my global eviction policy. The `VecDeque` allows `select_page` to iterate over every frame in the system to find the best eviction candidate based on a global LRU-like criterion.

## Paging To And From Disk

#### DATA STRUCTURES

> D1: Copy here the **declaration** of each new or changed struct, enum type, and global variable. State the purpose of each within 30 words.

(Same as the answer for C1.)

#### ALGORITHMS

> D2: When a frame is required but none is free, some frame must be evicted. Describe your code for choosing a frame to evict.

My eviction logic is in `select_page`. It locks the global `SwapManager` and iterates through the `frame_table`. It searches for the entry with the smallest `time` timestamp that is not currently `busy`. This `time` is assigned when a frame is allocated, so the minimum value approximates the least recently used frame. Once found, it marks the frame as `busy` and returns its identity (`stored_va`, `tid`) for the `swapout` procedure.

> D3: When a process P obtains a frame that was previously used by a process Q, how do you adjust the page table (and any other data structures) to reflect the frame Q no longer has?

When a frame from process Q is evicted for process P:
1.  **For Process Q:** The `swapout_pt` function finds Q's page table, invalidates the PTE for the evicted page, and saves the PTE's flags. It then updates Q's `swap_table` (SPT), changing the page's state to `Swapped` and recording its new location in the swap file. The old entry in the global `frame_table` is removed.
2.  **For Process P:** Process P receives the now-free physical frame from `UserPool`. It then updates its own page table to map its faulting virtual address to this physical frame, setting the Present bit and other necessary flags. A new entry for this mapping is created in the global `frame_table`.

#### SYNCHRONIZATION

> D5: Explain the basics of your VM synchronization design. In particular, explain how it prevents deadlock. (Refer to the textbook for an explanation of the necessary conditions for deadlock.)

My design is based on fine-grained locking and an optimistic retry mechanism to avoid deadlock.
*   **Mutual Exclusion:** This is guaranteed by `Mutex` and `Spin` locks.
*   **Hold and Wait:** My design explicitly breaks this. The main loop in the page fault handler does not hold locks while waiting for a resource. If `swapin` fails because no pages are free, it releases locks, attempts to create a free page via `select_page`/`swapout`, and then retries the entire `swapin` attempt.
*   **No Preemption (of locks):** While threads holding locks can be preempted by the scheduler, the lock itself ensures the critical section is not entered by another thread.
*   **Circular Wait:** I avoid circular wait by maintaining a simple lock hierarchy (e.g., global `GLB_SWM` lock, then per-process locks if needed) and by not having complex operations that require holding multiple locks for extended periods. The retry mechanism is key to breaking potential hold-and-wait cycles.

> D6: A page fault in process P can cause another process Q's frame to be evicted. How do you ensure that Q cannot access or modify the page during the eviction process? How do you avoid a race between P evicting Q's frame and Q faulting the page back in?

1.  **Preventing Access:** When P's `select_page` chooses Q's frame, it is marked `busy`. Then, `swapout_pt` invalidates the PTE in Q's page table. From that moment on, any access by Q to that page will trigger a new page fault, preventing it from accessing the frame's memory while it's being written to disk.
2.  **Avoiding the Race:** My design handles this race via optimistic retries. If P is evicting Q's frame and Q faults on it simultaneously, one will fail. For instance, Q's `swapin` will fail if no free frame is available yet. If Q's `exit()` preempts P's `swapout` and removes the frame's metadata, P's `swapout` will return early, causing its `handler` to retry. The failing process simply loops and re-evaluates, avoiding deadlock.

> D7: Suppose a page fault in process P causes a page to be read from the file system or swap. How do you ensure that a second process Q cannot interfere by e.g. attempting to evict the frame while it is still being read in?

When P's `swapin` begins, it first allocates a free frame from `UserPool`. It then immediately calls `register` to create an entry in the global `frame_table` for this new frame. Crucially, the `register` function can set the frame's initial state, and my logic ensures it is effectively considered "busy" or unevictable until `swapin` completes. In my implementation, `select_page` only chooses from existing, non-busy frames, so a newly allocated frame for a `swapin` operation is not yet a candidate for eviction.

> D8: Explain how you handle access to paged-out pages that occur during system calls. Do you use page faults to bring in pages (as in user programs), or do you have a mechanism for "locking" frames into physical memory, or do you use some other design? How do you gracefully handle attempted accesses to invalid virtual addresses?

My design uses the same page fault mechanism. If a system call accesses a user pointer that corresponds to a paged-out page, it will trigger a page fault in the kernel. The page fault handler is designed to be re-entrant. It will execute the normal `swapin` procedure to bring the page back into memory. Once the page is present, the faulting instruction within the system call is re-executed, and the system call continues. For truly invalid addresses, the handler would detect this and terminate the user process with an error.

#### RATIONALE

> D9: A single lock for the whole VM system would make synchronization easy, but limit parallelism. On the other hand, using many locks complicates synchronization and raises the possibility for deadlock but allows for high parallelism. Explain where your design falls along this continuum and why you chose to design it this way.

My design is positioned towards the "many locks, high parallelism" end of the spectrum, but it mitigates the risk of deadlock by adopting an optimistic, non-blocking-style synchronization strategy.

I use a global lock for the `SwapManager` and per-process locks for their respective `swap_table`s, allowing for concurrent operations. The key to my design is the retry loop in the page fault handler. Instead of using complex locking protocols to prevent all race conditions, my system allows them to occur but detects the resulting failure (e.g., `swapin` failing because a page wasn't freed as expected). Upon failure, it simply retries the operation.

I chose this design because it avoids the immense difficulty of proving a complex, multi-lock system to be deadlock-free. It trades a potential, rare performance penalty from retries ("livelock") for a much simpler and more robust system that is guaranteed not to deadlock.
