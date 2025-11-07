use crate::mem::{allocdata::*, VM_OFFSET};
use crate::sync::lazy;
use crate::thread::current;
use crate::OsError;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use core::ffi::CStr;
use core::mem::size_of;
use core::{panic, ptr};
use mem::palloc::UserPool;
use mem::PTEFlags;
use mem::PG_SIZE;
use PhysAddr;

const MAX_STACK: usize = 0x800000;
pub static mut SP: usize = 0;

pub fn check_ptr_writable(va: usize) -> bool {
    let thread = current();
    let pt_ref = thread.pagetable.as_ref().unwrap().lock();
    let ptentry = pt_ref.get_pte(va);
    return match ptentry {
        Some(entry) => entry.is_writable() | !entry.is_valid(),
        None => true,
    };
}

pub fn check_ptr_valid(va: usize) -> bool {
    let thread = current();
    let pt_ref = thread.pagetable.as_ref().unwrap().lock();
    let ptentry = pt_ref.get_pte(va);
    if ptentry.is_none() || !ptentry.unwrap().is_valid() {
        unsafe {
            if (current().stack_base.unwrap_or(0) - va <= MAX_STACK && va >= SP) {
                kprintln!(
                    "ptr check: va at {:x}, sp at {:x}, stack base at {:x}",
                    va,
                    SP,
                    current().stack_base.unwrap_or(0)
                );
                return true;
            }
            return false;
        }
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
    if !check_ptr_valid(ptr as usize) {
        return Err(OsError::BadPtr);
    }
    let mut head = ptr.wrapping_add(1);
    let mut checked_len = 1;
    while (checked_len < len) {
        // kprintln!("CNT");
        if !check_ptr_valid(head as usize) {
            return Err(OsError::BadPtr);
        }
        checked_len += PG_SIZE;
        head = head.wrapping_add(PG_SIZE);
    }
    Ok(())
}

pub fn check_slice_writable(ptr: *const u8, len: usize) -> Result<(), OsError> {
    for i in 0..len {
        if !check_ptr_writable(ptr.wrapping_add(i) as usize) {
            kprintln!("SLICE CHECK FAILED");
            return Err(OsError::InvalidFileMode);
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

pub fn alloc_from_pool(addr: usize) {
    let va_alloc = unsafe { UserPool::alloc_pages(1) };
    let thread = current();
    let mut pageinfo = thread.page_info.lock();
    pageinfo.push(PageInfo {
        va: va_alloc as usize,
        page_type: AllocType::Stack,
    });
    let mut flag = PTEFlags::V;
    flag.set(PTEFlags::R, true);
    flag.set(PTEFlags::U, true);
    flag.set(PTEFlags::W, true);
    flag.set(PTEFlags::V, true);
    flag.set(PTEFlags::D, false);
    current().pagetable.as_ref().unwrap().lock().map(
        PhysAddr::from(va_alloc),
        addr - (addr % PG_SIZE),
        1,
        flag,
    );
}
