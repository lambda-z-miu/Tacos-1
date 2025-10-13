const O_CREATE: usize = 0x200;
const O_TRUNC: usize = 0x400;

use core::i32;

use alloc::string::String;

use crate::fs::disk::Path;
use crate::userproc;
use crate::{fs::*, thread::current};

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
    let file = disk::DISKFS.open(disk::Path::from(&path as &str));
    if let Ok(file) = file {
        return userproc::execute(file, argv);
    } else {
        return -1;
    }
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
