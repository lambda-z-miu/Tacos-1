use crate::sync::Lock;
use crate::trap::flags::FdFlags;
use crate::trap::util::*;
use crate::OsError;
use core::fmt::{Display, Error};
use core::i32;
use core::slice::{from_raw_parts, from_raw_parts_mut};

use alloc::string::{String, ToString};
use riscv::register::fcsr::Flag;

use crate::fs::disk::{Path, DISKFS};
use crate::io::{Read, Seek, Write};
use crate::{fs::*, thread::current};
use crate::{sbi, thread, userproc};
use alloc::vec::Vec;
use core::ffi::CStr;
use core::ptr;
use core::sync::atomic::Ordering::SeqCst;

pub fn exec_handler(args0: usize, args1: usize) -> Result<isize, OsError> {
    // check validity of argument
    check_str_valid(args0 as *const u8)?;
    if args1 as usize % 8 != 0 {
        return Err(OsError::UnAlignedAccess);
    }

    // parse argument
    let path_str = c_str_to_string(args0 as *const u8);
    let argv = parse_arg(args1 as *const *const u8);

    // open elf and try to execute
    if !disk::Path::exists(disk::Path::from(&path_str as &str)) {
        return Err(OsError::FileNotExist);
    }
    let file = disk::DISKFS.open(disk::Path::from(&path_str as &str))?;
    return Ok(userproc::execute(file, argv));
}

pub fn read_handler(fd: u32, buf: *mut u8, len: usize) -> Result<isize, OsError> {
    // TODO: HOW to read stdin?
    //special cases
    if fd == 1 || fd == 2 {
        return Err(OsError::PermissionDenied); // cannot read stdout
    }
    if len == 0 {
        return Ok(0); //zero reading is permited
    }
    check_slice_valid(buf, len)?;
    check_slice_writable(buf, len)?;

    let thread = current();
    let mut fd_map = thread.fd.lock();
    let file = fd_map.get_mut(&fd).ok_or(OsError::FileNotExist)?; // file not exist
    file.1.read_permision()?; // file cannot be read
    unsafe {
        // kprintln!("READ HAPPENED");
        let size = file.0.read(from_raw_parts_mut(buf, len))?;
        return Ok(size as isize);
    }
}

pub fn write_handler(fd: u32, buf: *const u8, len: usize) -> Result<isize, OsError> {
    check_slice_valid(buf, len)?;
    if fd == 1 || fd == 2 {
        let i = buf;
        for j in 0..len {
            unsafe {
                kprint!("{}", (*i.wrapping_add(j)) as char);
            }
        }
    }

    //special cases
    if fd == 0 {
        return Err(OsError::PermissionDenied); // cannot write in stdin
    }
    if len == 0 {
        return Ok(0); //zero reading is permited
    }
    /*
        for i in 0..len {
            unsafe {
                kprintln!("WRITING {}", *(buf.wrapping_add(i)));
            }
        }
    */
    let thread = current();
    let mut fd_map = thread.fd.lock();
    let file = fd_map.get_mut(&fd).ok_or(OsError::FileNotExist)?; // file not exist
    file.1.write_permision()?; // permision denied
    unsafe {
        let size = file.0.write(from_raw_parts(buf, len))?;
        return Ok(size as isize);
    }
}

pub struct Fstat {
    pub ino: u64,
    pub size: u64,
}

impl Fstat {
    pub fn zeroed() -> Self {
        Fstat { ino: 0, size: 0 }
    }
}

pub fn fstat_handler(fd: u32, buf: *mut Fstat) -> isize {
    if check_str_valid(buf as *const u8).is_err() {
        return -1;
    };

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
            // kprintln!("METADATA: ino {}, size {}", (*buf).ino, (*buf).size);
        }
        return 0;
    }
    return -1;
}

pub fn open_handler(path: usize, flag: usize) -> Result<isize, OsError> {
    let fdflag = FdFlags { flag: flag };
    // kprintln!("Flag={}", flag);

    check_str_valid(path as *const u8)?;
    let path = c_str_to_string(path as *const u8);

    if path == "".to_string() {
        return Err(OsError::BadPtr);
    }

    let path_sys: disk::Path = disk::Path::from(&path as &str);
    if disk::Path::exists(disk::Path::from(&path as &str)) {
        //file exists
        let file_opened = disk::DISKFS.open(path_sys)?;
        // opened file
        let new_fd = current().get_fresh_fd();
        let thread = current();
        let mut fd_map = thread.fd.lock();
        fd_map.insert(new_fd, (file_opened, fdflag));
        return Ok(new_fd as isize);
    } else {
        // file does not exits
        fdflag.get_create()?;
        let file_opened = disk::DISKFS.create(path_sys)?;
        let new_fd = current().get_fresh_fd();
        let thread = current();
        let mut fd_map = thread.fd.lock();
        fd_map.insert(new_fd, (file_opened, fdflag));
        return Ok(new_fd as isize);
    }
}

pub fn close_handler(fd: u32) -> Result<isize, OsError> {
    if (fd == 0) | (fd == 1) | (fd == 2) {
        // special cases
        // TODO: how to close stdio file?
        return Ok(0);
    }

    let thread = current();
    let mut mmap_info = thread.mmap_info.lock();
    for i in mmap_info.iter_mut() {
        if i.fd == fd {
            i.need_close = true;
            return Ok(0);
        }
    }
    let mut fd_map = thread.fd.lock();
    let file = fd_map.get(&fd).ok_or(OsError::FileNotExist)?;
    disk::DISKFS.close(file.0.clone());
    fd_map.remove(&fd);
    return Ok(0);
}

pub fn seek_handler(fd: u32, pos: u32) -> isize {
    let thread = current();
    let mut fd_map = thread.fd.lock();
    let file = fd_map.get_mut(&fd);
    if let Some(file) = file {
        file.0.set_pos(pos);
        return 0;
    }
    return -1;
}

pub fn tell_handler(fd: u32) -> Result<isize, OsError> {
    let thread = current();
    let mut fd_map = thread.fd.lock();
    let file = fd_map.get_mut(&fd).ok_or(OsError::FileNotExist)?;
    let pos = file.0.pos()?;
    return Ok(*pos as isize);
}

pub fn remove_handler(path: usize) -> Result<isize, OsError> {
    check_str_valid(path as *const u8)?;
    let path = c_str_to_string(path as *const u8);

    if path == "".to_string() {
        return Err(OsError::BadPtr);
    }

    let path_sys: disk::Path = disk::Path::from(&path as &str);
    DISKFS.remove(path_sys)?;
    return Ok(0);
}

pub fn mkdir_handler(path: usize) -> Result<isize, OsError> {
    check_str_valid(path as *const u8)?;
    let path = c_str_to_string(path as *const u8);

    if path == "".to_string() {
        return Err(OsError::BadPtr);
    }

    let path_sys: disk::Path = disk::Path::from(&path as &str);
    DISKFS.create_dir(path_sys)?;
    return Ok(0);
}

pub fn chdir_handler(path: usize) -> Result<isize, OsError> {
    check_str_valid(path as *const u8)?;
    let path = c_str_to_string(path as *const u8);

    if path == "".to_string() {
        return Err(OsError::BadPtr);
    }

    let path_sys: disk::Path = disk::Path::from(&path as &str);
    DISKFS.change_dir(path_sys)?;
    return Ok(0);
}
