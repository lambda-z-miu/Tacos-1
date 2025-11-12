use core::panic;

use crate::fs::disk::Swap;
use crate::mem::PTEFlags;
use crate::mem::PG_SIZE;
use crate::sync::Lazy;
use crate::sync::Mutex;
use crate::thread;
use crate::thread::current;
use alloc::collections::btree_map::BTreeMap;
use alloc::collections::vec_deque::VecDeque;

#[derive(PartialEq, Eq, Clone, Copy)]
pub enum MemState {
    InMem,
    Swapped,
    Exe,
}
#[derive(Clone)]
pub struct SwapTableEntry {
    pub addr: usize,
    pub state: MemState,
    pub flags: PTEFlags,
    pub file_off: Option<u32>,
}

static BLOCK_MAP: Lazy<Mutex<BTreeMap<u32, usize>>> = Lazy::new(|| Mutex::new(BTreeMap::new()));

pub fn get_slot() -> u32 {
    let mut pos = 0;
    while (true) {
        let mut blockmap = BLOCK_MAP.lock();
        if blockmap.get(&pos).is_none() {
            return pos;
        }
        pos += (PG_SIZE as u32);
    }
    panic!("unreachable");
}

pub fn clean_ste(page: usize) {
    let thread = current();
    let mut swap_table = thread.swap_table.lock();
    swap_table.retain(|x| x.addr != page);
}

pub fn register(page: usize, state: MemState, flags: PTEFlags, file_off: Option<u32>) {
    let thread = current();
    let mut swap_table = thread.swap_table.lock();
    for i in swap_table.iter() {
        if i.addr == page {
            panic!("conflic item");
        }
    }
    // kprintln!("INSERTED IN SWAP TABLE");

    if state == MemState::InMem && file_off.is_some() {
        let mut blockmap = BLOCK_MAP.lock();
        blockmap.remove(&file_off.unwrap());
    } else if state == MemState::Swapped {
        let mut blockmap = BLOCK_MAP.lock();
        blockmap.insert(file_off.unwrap(), page);
    }

    swap_table.push_back(SwapTableEntry {
        addr: page,
        state,
        flags,
        file_off: file_off,
    });
}

pub fn get_page_pos(page: usize) -> Option<u32> {
    let thread = current();
    let mut swap_table = thread.swap_table.lock();
    for i in 0..swap_table.len() {
        if swap_table[i].addr == page {
            return swap_table[i].file_off;
        }
    }
    return None;
}

pub fn get_page_flags(page: usize) -> PTEFlags {
    let thread = current();
    let mut swap_table = thread.swap_table.lock();
    for i in swap_table.iter() {
        if i.addr == page {
            return i.flags;
        }
    }
    panic!("Trying to swap in a file that is not in swap file");
}

pub fn get_page_state(page: usize) -> MemState {
    let thread = current();
    let mut swap_table = thread.swap_table.lock();
    for i in swap_table.iter() {
        if i.addr == page {
            return i.state;
        }
    }
    panic!("NOT FOUND");
}

pub fn select_page() -> usize {
    static mut POSMEM: usize = 0;
    let thread = current();
    let mut swap_table = thread.swap_table.lock();
    let len = swap_table.len();
    unsafe {
        for i in POSMEM..len {
            if swap_table[i].state == MemState::InMem {
                return swap_table[i].addr;
            }

            POSMEM += 1;
        }

        POSMEM = 0;

        for i in 0..len {
            if swap_table[i].state == MemState::InMem {
                return swap_table[i].addr;
            }

            POSMEM += 1;
        }
    }

    panic!("select page error");
}
