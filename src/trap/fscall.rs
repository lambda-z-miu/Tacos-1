use crate::trap::flags::FdFlags;
use core::i32;
use core::slice::{from_raw_parts, from_raw_parts_mut};

use alloc::string::String;
use riscv::register::fcsr::Flag;

use crate::fs::disk::Path;
use crate::io::{Read, Write};
use crate::{fs::*, thread::current};
use crate::{thread, userproc};
use core::ptr;

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

        if !file.1.read_permision() {
            // no reading permision
            return -1;
        }

        unsafe {
            let size = file.0.read(from_raw_parts_mut(buf, len));
            if size.is_err() {
                // cannot read
                return -1;
            }
            let ret = size.unwrap() as isize;
            kprintln!("read {} chars", ret);
            return ret;
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
    //---------------------------------------------------

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
        if !file.1.write_permision() {
            // no write permision
            kprintln!("NO WR Flag={}", file.1.flag);
            return -1;
        }
        unsafe {
            let size = file.0.write(from_raw_parts(buf, len));
            if size.is_err() {
                // cannot write
                return -1;
            }
            let ret = size.unwrap() as isize;
            kprintln!("WRITECALLED {}", ret);
            return ret;
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

pub fn open_handler(path: String, flag: usize) -> isize {
    let fdflag = FdFlags { flag: flag };
    kprintln!("Flag={}", flag);

    let path_sys: disk::Path = disk::Path::from(&path as &str);
    if disk::Path::exists(disk::Path::from(&path as &str)) {
        //file exists
        if let Ok(file_opened) = disk::DISKFS.open(path_sys) {
            // opened file
            let new_fd = current().get_fresh_fd();
            kprintln!("OPENED:EXISTED {}", new_fd);
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
                kprintln!("OPENED:CREATED {}", new_fd);
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
