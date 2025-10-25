use crate::thread::current;
use crate::OsError;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use core::ffi::CStr;
use core::mem::size_of;
use core::ptr;

const MAX_STACK: usize = 0x800000;

pub fn check_ptr_valid(va: usize) -> bool {
    // kprintln!("CHECKVALID CALLED at {}", va);
    let thread = current();
    let pt_ref = thread.pagetable.as_ref().unwrap().lock();
    let ptentry = pt_ref.get_pte(va);
    if ptentry.is_none() || !ptentry.unwrap().is_valid() {
        kprintln!("{:x}", current().stack_base.unwrap_or(0xbeef));
        if current().stack_base.unwrap_or(0) - va < MAX_STACK {
            kprintln!("CALLED");
            return true;
        }
        return false;
    }
    return true;
}

pub fn check_str_valid(ptr: *const u8) -> Result<(), OsError> {
    let mut cur = ptr;
    while true {
        if !check_ptr_valid(cur as usize) {
            return Err(OsError::BadPtr);
        }
        unsafe {
            if (*cur) == '\0' as u8 {
                return Ok(());
            }
        }
        cur = cur.wrapping_add(1);
    }
    unreachable!();
}

pub fn check_slice_valid(ptr: *const u8, len: usize) -> Result<(), OsError> {
    for i in 0..len {
        if !check_ptr_valid(ptr.wrapping_add(i) as usize) {
            return Err(OsError::BadPtr);
        }
    }
    Ok(())
}

pub fn c_str_to_string(c_string: *const u8) -> String {
    unsafe {
        let c_str = CStr::from_ptr(c_string);
        let bytes = c_str.to_bytes();
        let rust_str = str::from_utf8_unchecked(bytes);
        String::from(rust_str)
    }
}

pub fn parse_arg(argv: *const *const u8) -> Vec<String> {
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
