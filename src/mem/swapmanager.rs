use core::fmt::Display;
use core::hash::BuildHasher;
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

pub static mut GLB_SWM: Lazy<Mutex<SwapManager>> = Lazy::new(|| {
    Mutex::new(SwapManager {
        swaptable: VecDeque::new(),
        blockmap: BTreeMap::new(),
    })
});

pub struct SwapManager {
    pub swaptable: VecDeque<SwapTableEntry>,
    blockmap: BTreeMap<u32, (isize, usize)>,
}

impl Display for MemState {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            MemState::InMem => write!(f, "InMem"),
            MemState::Exe => write!(f, "Exe"),
            MemState::Swapped => write!(f, "InSwap"),
        }
    }
}
#[derive(Clone)]
pub struct SwapTableEntry {
    pub addr: (isize, usize),
    pub state: MemState,
    pub flags: PTEFlags,
    pub file_off: Option<u32>,
    pub kva: Option<usize>,
    // pub busy: bool,
}

pub fn get_slot() -> u32 {
    let mut pos = 0;
    unsafe {
        while (true) {
            let mut blockmap: &mut BTreeMap<u32, (isize, usize)> = &mut GLB_SWM.lock().blockmap;
            if blockmap.get(&pos).is_none() {
                blockmap.insert(pos, (current().id(), 0));
                return pos;
            }
            // kprintln!("insert at {}", pos);
            pos += (PG_SIZE as u32);
        }
    }
    panic!("unreachable");
}

pub fn clean_ste(page: (isize, usize)) {
    unsafe {
        let mut swap_table = &mut GLB_SWM.lock().swaptable;
        swap_table.retain(|x| x.addr != page);
    }
}

pub fn register(
    page: (isize, usize),
    state: MemState,
    flags: PTEFlags,
    file_off: Option<u32>,
    kva: Option<usize>,
) {
    let thread = current();
    let tid = thread.id();

    /*  kprintln!(
        "reg from {}, register {:x} as {} at {:x}",
        page.0,
        page.1,
        state,
        file_off.unwrap_or(0xdeedbeef)
    );*/
    unsafe {
        let mut glb_swm = GLB_SWM.lock();

        let mut swap_table = &mut glb_swm.swaptable;
        let mut filepos = file_off;
        for i in swap_table.iter() {
            if i.addr == page {
                panic!(
                    "conflic item addr {:x} of thread {}, original state {}, now {}",
                    i.addr.1, i.addr.0, i.state, state
                );
            }
        }
        swap_table.push_back(SwapTableEntry {
            addr: page,
            state,
            flags,
            file_off: filepos,
            kva: kva,
        });

        // kprintln!("INSERTED IN SWAP TABLE");

        if state == MemState::InMem && file_off.is_some() {
            let mut blockmap = &mut glb_swm.blockmap;
            blockmap.remove(&file_off.unwrap());
            filepos = None;
        } else if state == MemState::Swapped {
            let mut blockmap = &mut glb_swm.blockmap;
            blockmap.insert(file_off.unwrap(), page);
        }
    }
}

pub fn get_page_pos(page: (isize, usize)) -> Option<u32> {
    let thread = current();
    unsafe {
        let mut swap_table = &mut GLB_SWM.lock().swaptable;
        for i in 0..swap_table.len() {
            if swap_table[i].addr == page {
                return swap_table[i].file_off;
            }
        }
        return None;
    }
}

pub fn get_page_kva(page: (isize, usize)) -> Option<usize> {
    let thread = current();
    unsafe {
        let mut swap_table = &mut GLB_SWM.lock().swaptable;
        for i in 0..swap_table.len() {
            if swap_table[i].addr == page {
                return swap_table[i].kva;
            }
        }
    }
    return None;
}

pub fn get_page_flags(page: (isize, usize)) -> PTEFlags {
    let thread = current();
    unsafe {
        let mut swap_table = &mut GLB_SWM.lock().swaptable;
        for i in swap_table.iter() {
            if i.addr == page {
                return i.flags;
            }
        }
    }
    panic!("Trying to swap in a file that is not in swap file");
}

pub fn get_page_state(page: (isize, usize)) -> MemState {
    let thread = current();
    unsafe {
        let mut swap_table = &mut GLB_SWM.lock().swaptable;
        for i in swap_table.iter() {
            if i.addr == page {
                return i.state;
            }
        }
    }
    panic!("NOT FOUND");
}

pub fn select_page() -> (isize, usize) {
    static mut POSMEM: usize = 0;
    let thread = current();
    unsafe {
        let mut swap_table = &mut GLB_SWM.lock().swaptable;
        let len = swap_table.len();
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
