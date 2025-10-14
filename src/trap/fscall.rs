const O_CREATE: usize = 0x200;
const O_TRUNC: usize = 0x400;

use core::i32;
use core::slice::{from_raw_parts, from_raw_parts_mut};

use alloc::string::String;

use crate::fs::disk::Path;
use crate::io::{Read, Write};
use crate::{fs::*, thread::current};
use crate::{thread, userproc};
use core::ptr;

pub struct Flags {
    pub flag: usize,
}

impl Flags {
    /*
    fn write_permision(&self) -> bool {
        (self.flag & 0b10) != 0
    }*/

    fn get_create(&self) -> bool {
        (self.flag & O_CREATE) != 0
    }
}

pub fn exec_handler(path: String, argv: alloc::vec::Vec<String>) -> isize {
    if disk::Path::exists(disk::Path::from(&path as &str)) {
        let file = disk::DISKFS.open(disk::Path::from(&path as &str));
        if let Ok(file) = file {
            return userproc::execute(file, argv);
        }
    }
    return -1;
}

pub fn read_handler(fd: u32, buf: *mut u8, len: usize) -> isize {
    //special cases
    if fd == 1 {
        return -1; // cannot read stdout
    }
    if len == 0 {
        return 0; //zero reading is permited
    }

    let thread = current();
    let mut fd_map = thread.fd.lock();
    let file = fd_map.get_mut(&fd);
    if let Some(file) = file {
        // fd is valid?
        unsafe {
            let size = file.read(from_raw_parts_mut(buf, len)); // fd can be opened?
            if size.is_ok() {
                let ret = size.unwrap() as isize;
                return ret;
            }
            return -1;
        }
    }
    return -1;
}

pub fn write_handler(fd: u32, buf: *const u8, len: usize) -> isize {
    //DEBUG:
    let i = buf;
    for j in 0..len {
        unsafe {
            kprint!("{}", (*i.wrapping_add(j)) as char);
        }
    }
    kprintln!("");

    //special cases
    if fd == 0 {
        return -1; // cannot write in
    }
    if len == 0 {
        return 0; //zero reading is permited
    }

    let thread = current();
    let mut fd_map = thread.fd.lock();
    let file = fd_map.get_mut(&fd);
    if let Some(file) = file {
        // fd is valid?
        unsafe {
            let size = file.write(from_raw_parts(buf, len)); // fd can be opened?
            if size.is_ok() {
                return size.unwrap() as isize;
            }
            return -1;
        }
    }
    return -1;
}

pub struct Fstat {
    ino: u64,
    size: u64,
}

pub fn fstat_handler(fd: u32, buf: *mut Fstat) -> isize {
    let thread = current();
    let mut fd_map = thread.fd.lock();
    let file = fd_map.get_mut(&fd);
    if let Some(file) = file {
        // fd is valid?
        let (ino, size) = file.fstat();
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

pub fn open_handler(path: String, flag: Flags) -> isize {
    let path_sys: disk::Path = disk::Path::from(&path as &str);

    if disk::Path::exists(disk::Path::from(&path as &str)) {
        if let Ok(file_opened) = disk::DISKFS.open(path_sys) {
            let new_fd = current().get_fresh_fd();
            current().fd.lock().insert(new_fd, file_opened);
            return new_fd as isize;
        } else {
            // unable to open file
            return -1;
        }
    } else {
        // file does not exits
        if flag.get_create() {
            if let Ok(file_opened) = disk::DISKFS.create(path_sys) {
                let new_fd = current().get_fresh_fd();
                current().fd.lock().insert(new_fd, file_opened);
                kprintln!("REACHED {}", new_fd);
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
    let file = current().fd.lock().get(&fd).cloned();
    if let Some(file) = file {
        disk::DISKFS.close(file);
        current().fd.lock().remove(&fd);
        return 0;
    } else {
        if (fd == 0) | (fd == 1) | (fd == 2) {
            // how to close stdio file?
            return 0;
        }
        return -1;
    }
}
