use crate::mem::PTEFlags;
use crate::mem::PG_SIZE;
use crate::sync::Lazy;
use crate::sync::Mutex;
use alloc::collections::vec_deque::VecDeque;

#[derive(PartialEq, Eq, Clone, Copy)]
pub enum MemState {
    InMem,
    Swapped,
    Exe,
}

pub struct SwapTableEntry {
    pub addr: usize,
    pub state: MemState,
    pub flags: PTEFlags,
    pub file_off: u32,
}

pub static SWAP_TABLE: Lazy<Mutex<VecDeque<SwapTableEntry>>> =
    Lazy::new(|| Mutex::new(VecDeque::new()));

pub fn get_slot() -> u32 {
    let swap_table = SWAP_TABLE.lock();
    /*
    for i in 0..swap_table.len() {
        if swap_table[i].state == MemState::InMem {
            return (i * PG_SIZE) as u32;
        }
    }*/
    return (swap_table.len() * PG_SIZE) as u32;
}

pub fn register(page: usize, state: MemState, flags: PTEFlags, file_off: u32) {
    let mut swap_table = SWAP_TABLE.lock();
    for i in swap_table.iter_mut() {
        if i.addr == page {
            i.state = state;
            i.flags = flags;
            i.file_off = file_off;
            // kprintln!("page {:x}, state OVERWRITEN", page);
            return;
        }
    }
    swap_table.push_back(SwapTableEntry {
        addr: page,
        state,
        flags,
        file_off: file_off,
    });
}

pub fn get_page_pos(page: usize) -> Option<u32> {
    let swap_table = SWAP_TABLE.lock();
    for i in 0..swap_table.len() {
        if swap_table[i].addr == page {
            return Some((i * PG_SIZE) as u32);
        }
    }
    return None;
}

pub fn get_page_flags(page: usize) -> PTEFlags {
    let swap_table = SWAP_TABLE.lock();
    for i in swap_table.iter() {
        if i.addr == page {
            return i.flags;
        }
    }
    panic!("Trying to swap in a file that is not in swap file");
}

pub fn get_page_state(page: usize) -> MemState {
    let swap_table = SWAP_TABLE.lock();
    for i in swap_table.iter() {
        if i.addr == page {
            return i.state;
        }
    }
    panic!("NOT FOUND");
}

pub fn select_page() -> usize {
    static mut POSMEM: usize = 0;
    let swap_table = SWAP_TABLE.lock();
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
