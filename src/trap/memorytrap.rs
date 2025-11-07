use crate::mem::allocdata::*;
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
    pub need_close: bool,
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
    /*
    for i in current().mmap_info.lock().iter() {
        if check_overlap(fd, i.fd) {
            kprintln!("CALLED");
            return Err(OsError::OverlappingMMap);
        }
    }*/

    if check_page_overlap(addr as usize) {
        return Err(OsError::OverlappingMMap);
    }

    let pages_need = (size as usize + PG_SIZE - 1) / PG_SIZE;
    let mmap_id = MMAP_CNT.fetch_add(1, Ordering::SeqCst);

    // kprintln!("! {}", pages_need);

    let mmapitem = MmapData {
        mmap_id: mmap_id,
        addr: addr as usize,
        pages: pages_need,
        fd: fd,
        len: size,
        need_close: false,
    };

    let thread = current();
    let mut pageinfo = thread.page_info.lock();
    for i in 0..pages_need {
        pageinfo.push(PageInfo {
            va: addr as usize + PG_SIZE * i,
            page_type: AllocType::MemMap,
        });
    }

    current().add_mmap(mmapitem);
    /*
    kprintln!(
        "mmap id {} is {} ditry",
        mmap_id,
        current()
            .pagetable
            .as_ref()
            .unwrap()
            .lock()
            .get_pte(addr as usize)
            .unwrap()
            .is_dirty()
    );*/

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

fn check_page_overlap(va: usize) -> bool {
    kprintln!("called check page overlap at 0x{:x}", va);
    let thread = current();
    let page_info = thread.page_info.lock();
    for i in page_info.iter() {
        kprintln!("existing page at 0x{:x}", i.va);
        if (i.va == va) {
            return true;
        }
    }
    return false;
}

pub fn unmap_handler(mmap_id: u32) -> Result<isize, OsError> {
    let thread = current();
    let mut mmap_info = thread.mmap_info.lock();
    for i in 0..mmap_info.len() {
        if mmap_info[i].mmap_id == mmap_id {
            let item = mmap_info.remove(i);

            let base = item.addr;
            let mut dirty = false;
            for i in 0..item.pages {
                if let Some(found_page) = thread
                    .pagetable
                    .as_ref()
                    .unwrap()
                    .lock()
                    .get_pte(base + i * PG_SIZE)
                {
                    kprintln!(
                        "page {} dirty bit {}",
                        i,
                        found_page.is_dirty() && found_page.is_valid()
                    );
                    dirty |= (found_page.is_dirty() && found_page.is_valid());
                }
            }

            if dirty {
                kprintln!("WB CALLED");
                let pos_mem = fscall::tell_handler(item.fd)?;
                fscall::write_handler(item.fd, item.addr as *const u8, item.len as usize);
                if pos_mem >= 0 {
                    fscall::seek_handler(item.fd, pos_mem as u32);
                    return Ok(0);
                }
                if item.need_close == true {
                    fscall::close_handler(item.fd);
                }
            }
        }
    }
    return Err(OsError::MMapIDNotExist);
}
