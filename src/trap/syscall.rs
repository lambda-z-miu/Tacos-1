//! Syscall handlers
//!

#![allow(dead_code)]

use crate::fs::disk;
use crate::thread::current;
use crate::trap::{fscall, memorytrap, Frame};
use crate::{fs, sbi};
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use userproc;

/* -------------------------------------------------------------------------- */
/*                               SYSCALL NUMBER                               */
/* -------------------------------------------------------------------------- */

const SYS_HALT: usize = 1; // Halt the system
const SYS_EXIT: usize = 2; // Exit the current process
const SYS_EXEC: usize = 3;
const SYS_WAIT: usize = 4;
const SYS_REMOVE: usize = 5;
const SYS_OPEN: usize = 6;
const SYS_READ: usize = 7;
const SYS_WRITE: usize = 8;
const SYS_SEEK: usize = 9;
const SYS_TELL: usize = 10;
const SYS_CLOSE: usize = 11;
const SYS_FSTAT: usize = 12;
const SYS_MMAP: usize = 13;
const SYS_UNMAP: usize = 14;
const SYS_BRK: usize = 17;

pub fn syscall_handler(_id: usize, args: [usize; 3]) -> isize {
    let old = sbi::interrupt::set(false);
    /*
    kprintln!(
        "handler called 0x{:x} 0x{:x} 0x{:x} 0x{:x} from {}",
        _id,
        args[0],
        args[1],
        args[2],
        current().id()
    );*/
    let id = match _id {
        SYS_HALT => sbi::shutdown(),

        SYS_EXIT => userproc::exit(args[0] as isize),

        SYS_EXEC => fscall::exec_handler(args[0], args[1]).unwrap_or(-1),

        SYS_WAIT => userproc::wait(args[0] as isize).unwrap_or(-1),

        SYS_REMOVE => fscall::remove_handler(args[0] as usize).unwrap_or(-1),

        SYS_OPEN => fscall::open_handler(args[0], args[1]).unwrap_or(-1),

        SYS_READ => fscall::read_handler(args[0] as u32, args[1] as *mut u8, args[2]).unwrap_or(-1),

        SYS_WRITE => {
            fscall::write_handler(args[0] as u32, args[1] as *const u8, args[2]).unwrap_or(-1)
        }

        SYS_SEEK => fscall::seek_handler(args[0] as u32, args[1] as u32),

        SYS_TELL => fscall::tell_handler(args[0] as u32).unwrap_or(-1),

        SYS_CLOSE => fscall::close_handler(args[0] as u32).unwrap_or(-1),

        SYS_FSTAT => fscall::fstat_handler(args[0] as u32, args[1] as *mut fscall::Fstat),

        SYS_MMAP => memorytrap::mmap_handler(args[0] as u32, args[1] as *mut u8).unwrap_or(-1),

        SYS_UNMAP => memorytrap::unmap_handler(args[0] as u32).unwrap_or(-1),

        SYS_BRK => memorytrap::brk_handler(args[0]).unwrap_or(-1),

        _ => {
            kprintln!("unexpected syscall id: {}", _id);
            kprintln!("args {} {} {}", args[0], args[1], args[2]);
            -1
        }
    };
    sbi::interrupt::set(old);
    id
}
