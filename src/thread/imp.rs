//! Implementation of kernel threads

use crate::fs::disk::{DiskFs, DISKFS};
use crate::mem::allocdata::{self, PageInfo};
use crate::mem::swapmanager::SwapTableEntry;
use crate::trap::flags::FdFlags;
use alloc::boxed::Box;
use alloc::collections::vec_deque::VecDeque;
use alloc::collections::BTreeMap;
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::arch::global_asm;
use core::fmt::{self, Debug};
use core::sync::atomic::{AtomicIsize, AtomicU32, Ordering::SeqCst};
use fs::FileSys;

use crate::fs::File;
use crate::mem::{kalloc, kfree, PageTable, PG_SIZE};
use crate::sbi::interrupt;
use crate::sync::{sleep, Semaphore};
use crate::thread::{self, current, Manager};
use crate::trap::memorytrap::MmapData;
use crate::userproc::UserProc;

pub const PRI_DEFAULT: u32 = 31;
pub const PRI_MAX: u32 = 63;
pub const PRI_MIN: u32 = 0;
pub const STACK_SIZE: usize = PG_SIZE * 4;
pub const STACK_ALIGN: usize = 16;
pub const STACK_TOP: usize = 0x80500000;
pub const MAGIC: usize = 0xdeadbeef;

pub static TID: AtomicIsize = AtomicIsize::new(0);

pub type Mutex<T> = crate::sync::Mutex<T, crate::sync::Intr>;
const MAX_FD: u32 = 32767;

/* --------------------------------- Thread --------------------------------- */
/// All data of a kernel thread
#[repr(C)]
pub struct Thread {
    tid: isize,
    name: &'static str,
    stack: usize,
    status: Mutex<Status>,
    pub context: Mutex<Context>,
    pub priority: AtomicU32,
    pub userproc: Option<UserProc>,
    pub pagetable: Option<Mutex<PageTable>>,
    pub exit_code: Mutex<Option<isize>>,
    pub children: Mutex<Vec<Arc<Thread>>>,
    pub completed: Semaphore,
    pub fd: Mutex<BTreeMap<u32, (File, FdFlags)>>,
    pub stack_base: Option<usize>,
    pub page_info: Mutex<Vec<PageInfo>>,
    pub mmap_info: Mutex<Vec<MmapData>>,
    pub swap_table: Mutex<BTreeMap<usize, SwapTableEntry>>,
}

impl Thread {
    pub fn new(
        name: &'static str,
        stack: usize,
        priority: u32,
        entry: usize,
        userproc: Option<UserProc>,
        pagetable: Option<PageTable>,
        stack_base: Option<usize>,
        page_info: Vec<PageInfo>,
        swap_table: BTreeMap<usize, SwapTableEntry>,
        thread_id: isize,
    ) -> Self {
        /// The next thread's id
        // for i in swap_table.iter() {
        /*kprintln!(
            "page at {:x} in swap table, fo {:x}",
            i.addr,
            i.file_off.unwrap_or(0xbeef)
        );*/
        // }
        Thread {
            tid: thread_id,
            name,
            stack,
            status: Mutex::new(Status::Ready),
            context: Mutex::new(Context::new(stack, entry)),
            priority: AtomicU32::new(priority),
            userproc,
            pagetable: pagetable.map(Mutex::new),
            exit_code: Mutex::new(None),
            children: Mutex::new(Vec::new()),
            completed: Semaphore::new(0),
            fd: Mutex::new(BTreeMap::new()),
            stack_base: stack_base,
            page_info: Mutex::new(page_info),
            mmap_info: Mutex::new(Vec::new()),
            swap_table: Mutex::new(swap_table),
        }
    }

    pub fn get_fresh_fd(&self) -> u32 {
        for i in 3..=MAX_FD {
            if self.fd.lock().get_key_value(&i).is_none() {
                return i;
            }
        }
        panic!("TOO MANY FD");
    }

    pub fn id(&self) -> isize {
        self.tid
    }

    pub fn name(&self) -> &'static str {
        self.name
    }

    pub fn status(&self) -> Status {
        *self.status.lock()
    }

    pub fn set_status(&self, status: Status) {
        *self.status.lock() = status;
    }

    pub fn context(&self) -> *mut Context {
        (&*self.context.lock()) as *const _ as *mut _
    }

    pub fn overflow(&self) -> bool {
        unsafe { (self.stack as *const usize).read() != MAGIC }
    }

    pub fn clean_fd(&self) {
        for i in self.fd.lock().iter() {
            DISKFS.close(i.1 .0.clone());
        }
    }

    pub fn add_mmap(&self, mmapitem: MmapData) {
        self.mmap_info.lock().push(mmapitem);
    }

    pub fn add_page(&self, pageitem: PageInfo) {
        self.page_info.lock().push(pageitem);
    }
}

impl Debug for Thread {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt.write_fmt(format_args!(
            "{}({})[{:?}]",
            self.name(),
            self.id(),
            self.status(),
        ))
    }
}

impl Drop for Thread {
    fn drop(&mut self) {
        #[cfg(feature = "debug")]
        kprintln!("[THREAD] {:?}'s resources are released", self);

        kfree(self.stack as *mut _, STACK_SIZE, STACK_ALIGN);
        if let Some(pt) = &self.pagetable {
            unsafe { pt.lock().destroy() };
        }
    }
}

/* --------------------------------- BUILDER -------------------------------- */
pub struct Builder {
    priority: u32,
    name: &'static str,
    function: usize,
    userproc: Option<UserProc>,
    pagetable: Option<PageTable>,
    stack_end: Option<usize>,
    page_info: Vec<PageInfo>,
    swap_table: BTreeMap<usize, SwapTableEntry>,
    thread_id: isize,
}

impl Builder {
    pub fn new<F>(function: F) -> Self
    where
        F: FnOnce() + Send + 'static,
    {
        // `*mut dyn FnOnce()` is a fat pointer, box it again to ensure FFI-safety.
        let function: *mut Box<dyn FnOnce()> = Box::into_raw(Box::new(Box::new(function)));

        Self {
            priority: PRI_DEFAULT,
            name: "Default",
            function: function as usize,
            userproc: None,
            pagetable: None,
            stack_end: None,
            page_info: Vec::new(),
            swap_table: BTreeMap::new(),
            thread_id: 0xbeef, // dummy
        }
    }

    pub fn pageinfo(mut self, page_record: Vec<PageInfo>) -> Self {
        self.page_info = page_record;
        self
    }

    pub fn thread_id(mut self, thread_id: isize) -> Self {
        self.thread_id = thread_id;
        self
    }

    pub fn swaptable(mut self, swaptable: BTreeMap<usize, SwapTableEntry>) -> Self {
        self.swap_table = swaptable;
        self
    }

    pub fn priority(mut self, priority: u32) -> Self {
        self.priority = priority;
        self
    }

    pub fn name(mut self, name: &'static str) -> Self {
        self.name = name;
        self
    }

    pub fn pagetable(mut self, pagetable: PageTable) -> Self {
        self.pagetable = Some(pagetable);
        self
    }

    pub fn set_stack(mut self, stack_end: usize) -> Self {
        self.stack_end = Some(stack_end);
        self
    }

    pub fn userproc(mut self, userproc: UserProc) -> Self {
        self.userproc = Some(userproc);
        self
    }

    pub fn build(self) -> Arc<Thread> {
        let stack = kalloc(STACK_SIZE, STACK_ALIGN) as usize;

        // Put magic number at the bottom of the stack.
        unsafe { (stack as *mut usize).write(MAGIC) };

        Arc::new({
            Thread::new(
                self.name,
                stack,
                self.priority,
                self.function,
                self.userproc,
                self.pagetable,
                self.stack_end,
                self.page_info,
                self.swap_table,
                self.thread_id,
            )
        })
    }

    /// Spawns a kernel thread and registers it to the [`Manager`].
    /// If it will run in the user environment, then two fields
    /// `userproc` and `pagetable` have to be set properly.
    ///
    /// Note that this function CANNOT be called during [`Manager`]'s initialization.
    pub fn spawn(self) -> Arc<Thread> {
        let new_thread = self.build();
        // *new_thread.page_info.lock() = self.page_info;
        current().children.lock().push(new_thread.clone());

        #[cfg(feature = "debug")]
        kprintln!("[THREAD] create {:?}", new_thread);

        Manager::get().register(new_thread.clone());

        // Off you go
        new_thread
    }
}

/* --------------------------------- Status --------------------------------- */
/// States of a thread's life cycle
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    /// Not running but ready to run
    Ready,
    /// Currently running
    Running,
    /// Waiting for an event to trigger
    Blocked,
    /// About to be destroyed
    Dying,
}

/* --------------------------------- Context -------------------------------- */
/// Records a thread's running status when it switches to another thread,
/// and when switching back, restore its status from the context.
#[repr(C)]
#[derive(Debug)]
pub struct Context {
    /// return address
    ra: usize,
    /// kernel stack
    sp: usize,
    /// callee-saved
    pub s: [usize; 12],
}

impl Context {
    fn new(stack: usize, entry: usize) -> Self {
        Self {
            ra: kernel_thread_entry as usize,
            // calculate the address of stack top
            sp: stack + STACK_SIZE,
            // s0 stores a thread's entry point. For a new thread,
            // s0 will then be used as the first argument of `kernel_thread`.
            s: core::array::from_fn(|i| if i == 0 { entry } else { 0 }),
        }
    }
}

/* --------------------------- Kernel Thread Entry -------------------------- */

extern "C" {
    /// Entrance of kernel threads, providing a consistent entry point for all
    /// kernel threads (except the initial one). A thread gets here from `schedule_tail`
    /// when it's scheduled for the first time. To understand why program reaches this
    /// location, please check on the initial context setting in [`Manager::create`].
    ///
    /// A thread's actual entry is in `s0`, which is moved into `a0` here, and then
    /// it will be invoked in [`kernel_thread`].
    fn kernel_thread_entry() -> !;
}

global_asm! {r#"
    .section .text
        .globl kernel_thread_entry
    kernel_thread_entry:
        mv a0, s0
        j kernel_thread
"#}

/// Executes the `main` function of a kernel thread. Once a thread is finished, mark
/// it as [`Dying`](Status::Dying).
#[no_mangle]
extern "C" fn kernel_thread(main: *mut Box<dyn FnOnce()>) -> ! {
    let main = unsafe { Box::from_raw(main) };

    interrupt::set(true);

    main();

    super::exit()
}
