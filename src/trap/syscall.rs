//! Syscall handlers
//!

#![allow(dead_code)]

use crate::fs::disk;
use crate::trap::fscall;
use crate::{fs, sbi};
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use core::ffi::CStr;
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

fn c_str_to_string(c_string: *const u8) -> String {
    unsafe {
        let c_str = CStr::from_ptr(c_string);
        let bytes = c_str.to_bytes();
        let rust_str = str::from_utf8_unchecked(bytes);
        String::from(rust_str)
    }
}

pub fn syscall_handler(_id: usize, args: [usize; 3]) -> isize {
    let old = sbi::interrupt::set(false);
    let id = match _id {
        SYS_READ => {
            // kprintln!("RHC {} {} {}", args[0], args[1], args[2]);
            fscall::read_handler(args[0] as u32, args[1] as *mut u8, args[2])
        }

        SYS_WRITE => fscall::write_handler(args[0] as u32, args[1] as *const u8, args[2]),

        SYS_HALT => sbi::shutdown(),

        SYS_EXIT => userproc::exit(args[0] as isize),

        SYS_OPEN => {
            // kprintln!("OHC {} {}", args[0], args[1]);
            let mut ret_code = -1;
            if args[0] != 0 {
                let path = c_str_to_string(args[0] as *const u8);
                if path != "".to_string() {
                    ret_code = fscall::open_handler(path, args[1] as usize);
                }
            }
            ret_code
        }

        SYS_EXEC => {
            let path_str = c_str_to_string(args[0] as *const u8);
            // kprintln!("EXECHC {}", path_str);
            let argv = parse_arg(args[1] as *const *const u8);
            fscall::exec_handler(path_str, argv)
        }

        SYS_WAIT => {
            // kprintln!("WAITCALLED");
            userproc::wait(args[0] as isize).unwrap_or(-1)
        }

        SYS_CLOSE => fscall::close_handler(args[0] as u32),
        SYS_SEEK => fscall::seek_handler(args[0] as u32, args[1] as u32),
        SYS_FSTAT => fscall::fstat_handler(args[0] as u32, args[1] as *mut fscall::Fstat),

        _ => {
            kprintln!("unexpected syscall id: {}", _id);
            kprintln!("args {} {} {}", args[0], args[1], args[2]);
            -1
        }
    };
    sbi::interrupt::set(old);
    id
}

fn parse_arg(argv: *const *const u8) -> Vec<String> {
    let mut args: Vec<String> = Vec::new();
    if argv.is_null() {
        return args;
    }

    unsafe {
        let mut strptr = argv;
        while !(*strptr).is_null() {
            let string = c_str_to_string(*strptr);
            args.push(string.clone());
            // kprintln!("!!{}", string.clone());
            strptr = strptr.add(1);
        }
    }
    args
}
