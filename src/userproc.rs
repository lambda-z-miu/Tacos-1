//! User process.
//!

mod load;

use alloc::string::String;
use alloc::vec::Vec;
use core::arch::{asm, global_asm};
use core::mem::{size_of, MaybeUninit};
use core::ptr::{self, copy, null, write};
use ptr::null_mut;
use riscv::register::sstatus;

use crate::fs::File;
use crate::mem::pagetable::KernelPgTable;
use crate::thread;
use crate::trap::{trap_exit_u, Frame};

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

    let exec_info = match load::load_executable(&mut file, &mut pt) {
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

    //
    let userproc = UserProc::new(file);

    // TODO: (Lab2) Pass arguments to user program

    kprintln!("DEBUG: frame.sepc {}", frame.sepc);

    pt.activate();

    let user_stack = frame.x[2];
    let mut index: *mut u8 = user_stack as *mut u8;
    let mut ptrs: Vec<*mut u8> = Vec::new();
    let argc = argv.len();

    for str in argv {
        let mut ended: String = str.clone();
        ended.push('\0');
        let ended_byte = ended.as_bytes();
        let ended_len = ended_byte.len();
        for j in ended_byte {
            kprint!("{:x} ", j)
        }
        kprintln!("\n len = {}", ended_len);

        unsafe {
            index = index.wrapping_sub(ended_len);
            ptrs.push(index);
            copy(ended_byte.as_ptr(), index as *mut u8, ended_len);
        }
    }

    let mut index_aligned: *mut u8 = (index as usize & !7) as *mut u8;
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

    thread::Builder::new(move || start(frame))
        .pagetable(pt)
        .userproc(userproc)
        .priority(63)
        .spawn()
        .id()
}

/// Exits a process.
///
/// Panic if the current thread doesn't own a user process.
pub fn exit(_value: isize) -> ! {
    // TODO: Lab2.
    thread::exit();
}

/// Waits for a child thread, which must own a user process.
///
/// ## Return
/// - `Some(exit_value)`
/// - `None`: if tid was not created by the current thread.
pub fn wait(_tid: isize) -> Option<isize> {
    // TODO: Lab2.
    Some(-1)
}

/// Initializes a user process in current thread.
///
/// This function won't return.
pub fn start(mut frame: Frame) -> ! {
    unsafe { sstatus::set_spp(sstatus::SPP::User) };
    frame.sstatus = sstatus::read();

    // Set kernel stack pointer to intr frame and then jump to `trap_exit_u()`.
    let kernal_sp = (&frame as *const Frame) as usize;

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
