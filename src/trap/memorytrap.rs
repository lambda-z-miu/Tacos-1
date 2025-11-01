use alloc::alloc::dealloc;

use crate::{
    mem::{self, palloc::UserPool, PG_SIZE},
    sync::{lazy, mutex, Lazy},
    thread::{self, current, MAGIC},
    trap::{fscall, util},
};
use core::{
    alloc::Layout,
    mem::size_of,
    ptr,
    sync::atomic::{AtomicU32, Ordering},
};

use crate::trap::util::*;
use crate::{
    mem::malloc,
    trap::fscall::{fstat_handler, tell_handler, Fstat},
    OsError,
};

pub struct MmapData {
    pub mmap_id: u32,
    pub addr: usize,
    pub pages: usize,
    pub fd: u32,
    pub len: u64,
}

impl MmapData {
    pub fn in_map(&self, va: usize) -> bool {
        return va >= self.addr && va < self.addr + self.pages * PG_SIZE;
    }
}

const MMAP_CNT: Lazy<AtomicU32> = Lazy::new(|| AtomicU32::new(0));

fn check_fd(fd: u32) -> Result<u64, OsError> {
    let mut buf: Fstat = Fstat::zeroed();

    let tmp = fstat_handler(fd, &mut buf as *mut Fstat);
    if tmp == -1 {
        return Err(OsError::FileNotExist);
    }
    let size = buf.size;
    if size == 0 {
        return Err(OsError::FileNotExist);
    }

    if fd == 0 || fd == 1 || fd == 2 {
        return Err(OsError::FileNotExist);
    }

    return Ok(size);
}

pub fn mmap_handler(fd: u32, addr: *mut u8) -> Result<isize, OsError> {
    let size = check_fd(fd)?;
    if addr as usize == 0 {
        return Err(OsError::BadPtr);
    }
    if addr as usize % PG_SIZE != 0 {
        return Err(OsError::BadPtr);
    }
    for i in current().mmap_info.lock().iter() {
        if check_overlap(fd, i.fd) {
            kprintln!("CALLED");
            return Err(OsError::OverlappingMMap);
        }
    }

    let pages_need = (size as usize + PG_SIZE - 1) / PG_SIZE;
    let mmap_id = MMAP_CNT.fetch_add(1, Ordering::SeqCst);

    kprintln!("! {}", pages_need);

    let mmapitem = MmapData {
        mmap_id: mmap_id,
        addr: addr as usize,
        pages: pages_need,
        fd: fd,
        len: size,
    };
    current().add_mmap(mmapitem);

    /*
    for i in 0..pages_need {
        util::alloc_from_pool((addr as usize) + i * PG_SIZE);
    }*/
    return Ok(mmap_id as isize);
}

fn check_overlap(fd1: u32, fd2: u32) -> bool {
    let thread = current();
    let fd_list = thread.fd.lock();
    let fd1_node = fd_list.get(&fd1).unwrap().0.inum();
    let fd2_node = fd_list.get(&fd2).unwrap().0.inum();
    return fd1_node == fd2_node;
}

pub fn unmap_handler(mmap_id: u32) -> Result<isize, OsError> {
    kprintln!("REACHED");
    let thread = current();
    let mut mmap_info = thread.mmap_info.lock();
    for i in 0..mmap_info.len() {
        if mmap_info[i].mmap_id == mmap_id {
            let item = mmap_info.remove(i);
            fscall::write_handler(item.fd, item.addr as *const u8, item.len as usize);
        }
    }
    return Err(OsError::MMapIDNotExist);
}
