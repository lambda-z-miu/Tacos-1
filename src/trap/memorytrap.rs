use alloc::alloc::dealloc;

use crate::{
    mem::{self, palloc::UserPool, PG_SIZE},
    sync::{lazy, mutex, Lazy},
    thread::MAGIC,
    trap::util,
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

const MMAP_CNT: Lazy<AtomicU32> = Lazy::new(|| AtomicU32::new(0));

pub fn mmap_handler(fd: u32, addr: *mut u8) -> Result<isize, OsError> {
    if fd == 0 || fd == 1 || fd == 2 {
        return Err(OsError::FileNotExist);
    }

    let mut buf: Fstat = Fstat::zeroed();
    let tmp = fstat_handler(fd, &mut buf as *mut Fstat);
    if tmp == -1 {
        return Err(OsError::FileNotExist);
    }
    let size = buf.size;
    if size == 0 {
        return Err(OsError::FileNotExist);
    }

    let pages_need = (size as usize + PG_SIZE - 1) / PG_SIZE;
    kprintln!("! {}", pages_need);
    for i in 0..pages_need {
        util::alloc_from_pool((addr as usize) + i * PG_SIZE);
    }
    let mmap_id = MMAP_CNT.fetch_add(1, Ordering::SeqCst);
    return Ok(mmap_id as isize);
}
