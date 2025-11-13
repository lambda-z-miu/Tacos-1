//! User process.
//!

mod load;

use alloc::borrow::ToOwned;
use alloc::collections::btree_map::Entry;
use alloc::collections::vec_deque::VecDeque;
use alloc::string::String;
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::arch::asm;
use core::mem::{size_of, MaybeUninit};
use ptr::*;
use riscv::register::sstatus;

use crate::fs::{self, File};
use crate::mem::pagetable::KernelPgTable;
use crate::mem::swapmanager::SwapTableEntry;
use crate::sync::{sleep, Lock};
use crate::thread::{self, current, Status, Thread, TID};
use crate::trap::{trap_exit_u, Frame};
use core::sync::atomic::Ordering::SeqCst;

pub struct UserProc {
    #[allow(dead_code)]
    bin: File,
}

impl UserProc {
    pub fn new(file: File) -> Self {
        Self { bin: file }
    }
}

/// Execute an object file with arguments.
///
/// ## Return
/// - `-1`: On error.
/// - `tid`: Tid of the newly spawned thread.
#[allow(unused_variables)]
pub fn execute(mut file: File, argv: Vec<String>) -> isize {
    #[cfg(feature = "debug")]
    kprintln!(
        "[PROCESS] Kernel thread {} prepare to execute a process with args {:?}",
        thread::current().name(),
        argv
    );

    // It only copies L2 pagetable. This approach allows the new thread
    // to access kernel code and data during syscall without the need to
    // switch pagetables.
    let mut pt = KernelPgTable::clone();
    let next_tid = TID.fetch_add(1, SeqCst);

    let exec_info = match load::load_executable(&mut file, &mut pt, next_tid) {
        Ok(x) => x,
        Err(_) => unsafe {
            pt.destroy();
            return -1;
        },
    };

    // Initialize frame, pass argument to user.
    let mut frame = unsafe { MaybeUninit::<Frame>::zeroed().assume_init() };
    frame.sepc = exec_info.entry_point;
    frame.x[2] = exec_info.init_sp;

    // Here the new process will be created.

    // TODO: (Lab2) Pass arguments to user program

    let mut tot_len = argv.len() * 8 + 8;
    for t in &argv {
        tot_len += t.len();
    }
    if tot_len > 4096 {
        panic!("TOO LONG ARGUMENT");
    }

    pt.activate();

    let user_stack = frame.x[2];
    let mut index: *mut u8 = user_stack as *mut u8;
    let mut ptrs: Vec<*mut u8> = Vec::new();
    let argc = argv.len();
    // kprintln!("{}", argc);

    for str in argv {
        let mut ended: String = str.clone();
        ended.push('\0');
        let ended_byte = ended.as_bytes();
        let ended_len = ended_byte.len();
        // kprintln!("\n len = {}", ended_len);

        unsafe {
            index = index.wrapping_sub(ended_len);
            ptrs.push(index);
            copy(ended_byte.as_ptr(), index as *mut u8, ended_len);
        }
    }

    let index_aligned: *mut u8 = (index as usize & !7) as *mut u8;
    assert_eq!((index_aligned <= index), true);
    assert_eq!(index_aligned as usize % 8, 0);
    assert_eq!(index <= index_aligned.wrapping_add(7), true);

    let argv_base: *mut *mut u8 = (index_aligned as usize - 8 * argc - 8) as *mut *mut u8;
    let mut index: *mut *mut u8 = argv_base;

    unsafe {
        for i in ptrs {
            // kprintln!("{:x}", i as usize);
            write(index, i);
            index = index.wrapping_add(1);
        }
        *(index as *mut *mut u8) = null_mut();
    }

    // kprintln!("MOVED ARGV");
    frame.x[10] = argc; // first arg reg
    frame.x[11] = argv_base as usize;
    frame.x[2] = argv_base as usize;

    /*
    match pt.get_pte(frame.sepc) {
        Some(x) => x.print(),
        None => kprintln!("No PTE found!"),
    };

    match pt.get_pte(0) {
        Some(x) => x.print(),
        None => kprintln!("No PTE found!"),
    };*/

    // activate old proc_pt
    if let Some(proc_pt) = thread::current().pagetable.as_ref() {
        proc_pt.lock().activate();
    }

    let userproc = UserProc::new(file);

    thread::Builder::new(move || start(frame))
        .pagetable(pt)
        .userproc(userproc)
        .set_stack((argv_base as usize))
        .tag(next_tid)
        .pageinfo(current().page_info.lock().to_vec())
        .spawn()
        .id()
}
/// Exits a process.
///
/// Panic if the current thread doesn't own a user process.
pub fn exit(_value: isize) -> ! {
    // TODO: Lab2.
    let thread = current();
    if thread.userproc.is_none() {
        panic!("cannot exit with no user process");
    } else {
        thread.completed.up();
        thread.exit_code.lock().replace(_value);
        thread.userproc.as_ref().unwrap().bin.allow_write();
        unsafe {
            thread.pagetable.as_ref().unwrap().lock().destroy();
        } // release all memory resources
        thread.clean_fd(); // release all file descriptor
                           // kprintln!("process exited with exit code {}", _value);
    }
    thread::exit();
}

/// Waits for a child thread, which must own a user process.
///
/// ## Return
/// - `Some(exit_value)`
/// - `None`: if tid was not created by the current thread.
use thread::sleep;
pub fn wait(tid: isize) -> Option<isize> {
    // TODO: Lab2.
    // kprintln!("WAITCALLED");
    let list = current().children.lock().clone();
    let mut found: Option<Arc<Thread>> = None;
    for i in list.clone().into_iter() {
        if i.id() == tid && i.status() != Status::Dying {
            found = Some(i.clone());
            break;
        }
    }

    let mut exit_code_get = None;
    if let Some(thread) = found {
        thread.completed.down();
        exit_code_get = thread.exit_code.lock().clone();
        thread.completed.up();
        // kprintln!("exited with {}", thread.exit_code.lock().unwrap_or(17))
    }

    exit_code_get
}

/// Initializes a user process in current thread.
///
/// This function won't return.
pub fn start(mut frame: Frame) -> ! {
    unsafe { sstatus::set_spp(sstatus::SPP::User) };
    frame.sstatus = sstatus::read();

    // Set kernel stack pointer to intr frame and then jump to `trap_exit_u()`.
    let kernal_sp = (&frame as *const Frame) as usize;
    // kprintln!("REACHED END OF START");
    // kprintln!("TID {}", current().id());

    unsafe {
        asm!(
            "mv sp, t0",
            "jr t1",
            in("t0") kernal_sp,
            in("t1") trap_exit_u as *const u8
        );
    }

    unreachable!();
}
