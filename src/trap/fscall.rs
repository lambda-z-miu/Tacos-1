use crate::sync::Lock;
use crate::trap::flags::FdFlags;
use core::i32;
use core::slice::{from_raw_parts, from_raw_parts_mut};

use alloc::string::{String, ToString};
use riscv::register::fcsr::Flag;

use crate::fs::disk::Path;
use crate::io::{Read, Write};
use crate::{fs::*, thread::current};
use crate::{sbi, thread, userproc};
use alloc::vec::Vec;
use core::ffi::CStr;
use core::ptr;
use core::sync::atomic::Ordering::SeqCst;

fn check_ptr_valid(va: usize) -> bool {
    // kprintln!("CHECKVALID CALLED at {}", va);
    let thread = current();
    let pt_ref = thread.pagetable.as_ref().unwrap().lock();
    let ptentry = pt_ref.get_pte(va);
    if ptentry.is_none() || !ptentry.unwrap().is_valid() {
        return false;
    }
    return true;
}

pub fn exec_handler(args0: usize, args1: usize) -> isize {
    if !check_str_valid(args0 as *const u8) {
        return -1;
    }
    if args1 as usize % 8 != 0 {
        // align-8
        return -1;
    }

    let path_str = c_str_to_string(args0 as *const u8);
    let argv = parse_arg(args1 as *const *const u8);

    if disk::Path::exists(disk::Path::from(&path_str as &str)) {
        let file = disk::DISKFS.open(disk::Path::from(&path_str as &str));
        if let Ok(file) = file {
            return userproc::execute(file, argv);
        }
    }
    return -1;
}

pub fn read_handler(fd: u32, buf: *mut u8, len: usize) -> isize {
    // TODO: HOW to read stdin?
    //special cases
    if fd == 1 {
        return -1; // cannot read stdout
    }
    if len == 0 {
        return 0; //zero reading is permited
    }
    if !check_str_valid(buf as *const u8) {
        return -1;
    }

    let thread = current();
    let mut fd_map = thread.fd.lock();
    let file = fd_map.get_mut(&fd);
    if let Some(file) = file {
        // fd is valid?
        if file.1.read_permision() {
            unsafe {
                let size = file.0.read(from_raw_parts_mut(buf, len));
                if let Ok(size) = size {
                    return size as isize;
                    // kprintln!("read {} chars", ret);
                }
            }
        }
    }
    return -1;
}

pub fn write_handler(fd: u32, buf: *const u8, len: usize) -> isize {
    if !check_str_valid(buf as *const u8) {
        kprintln!("F1");
        return -1;
    }

    if fd == 1 || fd == 2 {
        //kprintln!("called");

        let i = buf;
        for j in 0..len {
            unsafe {
                kprint!("{}", (*i.wrapping_add(j)) as char);
            }
        }
        // kprintln!("");
        // sbi::console_putchar(buf as usize);
    }

    //DEBUG:
    /*
    let i = buf;
    for j in 0..len {
        unsafe {
            kprint!("{}", (*i.wrapping_add(j)) as char);
        }
    }*/
    // kprintln!("");
    //---------------------------------------------------

    //special cases
    if fd == 0 {
        return -1; // cannot write in stdin
    }
    if len == 0 {
        return 0; //zero reading is permited
    }

    let thread = current();
    let mut fd_map = thread.fd.lock();
    let file = fd_map.get_mut(&fd);
    if let Some(file) = file {
        // fd is valid?
        if file.1.write_permision() {
            unsafe {
                let size = file.0.write(from_raw_parts(buf, len));
                if let Ok(size) = size {
                    return size as isize;
                }
            }
        }
    }
    return -1;
}

pub struct Fstat {
    ino: u64,
    size: u64,
}

pub fn fstat_handler(fd: u32, buf: *mut Fstat) -> isize {
    if !check_str_valid(buf as *const u8) {
        return -1;
    }

    let thread = current();
    let mut fd_map = thread.fd.lock();
    let file = fd_map.get_mut(&fd);
    if let Some(file) = file {
        // fd is valid?
        let (ino, size) = file.0.fstat();
        unsafe {
            *buf = Fstat {
                ino: ino as u64,
                size: size as u64,
            };
            kprintln!("METADATA: ino {}, size {}", (*buf).ino, (*buf).size);
        }
        return 0;
    }
    return -1;
}

pub fn open_handler(path: usize, flag: usize) -> isize {
    let fdflag = FdFlags { flag: flag };
    kprintln!("Flag={}", flag);

    if !check_str_valid(path as *const u8) {
        return -1;
    }
    let path = c_str_to_string(path as *const u8);

    if path == "".to_string() {
        return -1;
    }

    let path_sys: disk::Path = disk::Path::from(&path as &str);
    if disk::Path::exists(disk::Path::from(&path as &str)) {
        //file exists
        if let Ok(file_opened) = disk::DISKFS.open(path_sys) {
            // opened file
            let new_fd = current().get_fresh_fd();
            kprintln!("OPENED_EXISTED fd: {}", new_fd);
            let thread = current();
            let mut fd_map = thread.fd.lock();
            fd_map.insert(new_fd, (file_opened, fdflag));
            return new_fd as isize;
        } else {
            // unable to open file
            return -1;
        }
    } else {
        // file does not exits
        if fdflag.get_create() {
            if let Ok(file_opened) = disk::DISKFS.create(path_sys) {
                let new_fd = current().get_fresh_fd();
                let thread = current();
                let mut fd_map = thread.fd.lock();
                fd_map.insert(new_fd, (file_opened, fdflag));
                // kprintln!("OPENED_CREATED fd: {}", new_fd);
                return new_fd as isize;
            } else {
                // unable to create file
                return -1;
            }
        } else {
            return -1;
        }
    }
}

pub fn close_handler(fd: u32) -> isize {
    let thread = current();
    let mut fd_map = thread.fd.lock();
    let file = fd_map.get(&fd);
    if let Some(file) = file {
        disk::DISKFS.close(file.0.clone());
        fd_map.remove(&fd);
        return 0;
    } else {
        if (fd == 0) | (fd == 1) | (fd == 2) {
            // how to close stdio file?
            return 0;
        }
        return -1;
    }
}

pub fn seek_handler(fd: u32, pos: u32) -> isize {
    let thread = current();
    let mut fd_map = thread.fd.lock();
    let file = fd_map.get_mut(&fd);
    if let Some(file) = file {
        file.0.set_pos(pos);
    }
    return -1;
}

fn check_str_valid(ptr: *const u8) -> bool {
    let mut cur = ptr;
    while true {
        if !check_ptr_valid(cur as usize) {
            return false;
        }
        unsafe {
            if (*cur) == '\0' as u8 {
                return true;
            }
        }
        cur = cur.wrapping_add(1);
    }
    unreachable!();
}

fn c_str_to_string(c_string: *const u8) -> String {
    unsafe {
        let c_str = CStr::from_ptr(c_string);
        let bytes = c_str.to_bytes();
        let rust_str = str::from_utf8_unchecked(bytes);
        String::from(rust_str)
    }
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
