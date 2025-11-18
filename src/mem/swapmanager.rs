use core::panic;
use core::pin;
use core::sync::atomic::AtomicI64;

use crate::fs::disk::Swap;
use crate::mem::PTEFlags;
use crate::mem::PG_SIZE;
use crate::sync::Lazy;
use crate::sync::Lock;
use crate::sync::Mutex;
use crate::thread;
use crate::thread::current;
use crate::thread::manager;
use alloc::collections::btree_map::BTreeMap;
use alloc::collections::vec_deque::VecDeque;
pub static EXITLOCK: crate::sync::Spin = crate::sync::Spin::new();

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
    pub need_pin: bool,
}

pub static TIMESTAMP: AtomicI64 = AtomicI64::new(0);
pub struct FrameTableEntry {
    pub frame_va: usize,
    pub stored_va: usize,
    pub tid: isize,
    pub time: isize,
    pub busy: bool,
    pub ref_cnt: usize,
}

pub static mut GLB_SWM: Lazy<Mutex<SwapManager>> = Lazy::new(|| {
    Mutex::new(SwapManager {
        block_map: BTreeMap::new(),
        frame_table: VecDeque::new(),
    })
});

pub struct SwapManager {
    pub block_map: BTreeMap<u32, usize>,
    pub frame_table: VecDeque<FrameTableEntry>,
}

pub fn get_slot() -> u32 {
    let mut pos = 0;
    while (true) {
        unsafe {
            let mut blockmap = &mut GLB_SWM.lock().block_map;
            if blockmap.get(&pos).is_none() {
                blockmap.insert(pos, 0xbeef);
                return pos;
            }
        }
        pos += (PG_SIZE as u32);
    }
    panic!("unreachable");
}

pub fn get_pin_flag(swap_table: &VecDeque<SwapTableEntry>, page: usize) -> bool {
    let mut item = None;
    for i in swap_table.iter() {
        if i.addr == page {
            item = Some(i.clone());
            break;
        }
    }
    match item {
        Some(mut entry) => entry.need_pin,
        None => false,
    }
}

pub fn clean_ste(swap_table: &mut VecDeque<SwapTableEntry>, page: usize) {
    swap_table.retain(|x| x.addr != page);
}

// DEFAULT SWAP
pub fn register_swaptable(
    swap_table: &mut VecDeque<SwapTableEntry>,
    va: usize,
    state: MemState,
    flags: PTEFlags,
    file_off: Option<u32>,
    need_pin: bool,
) {
    for i in swap_table.iter() {
        if i.addr == va {
            panic!("conflic item");
        }
    }

    swap_table.push_back(SwapTableEntry {
        addr: va,
        state,
        flags,
        file_off: file_off,
        need_pin: need_pin,
    });
}

pub fn register(
    page_va: usize,
    tid: isize,
    state: MemState,
    flags: PTEFlags,
    file_off: Option<u32>,
    page_frame: Option<usize>,
    ref_cnt: usize,
) {
    /*
    kprintln!(
        "reg page {:x} state {} file offset {:x} in thread {},pgframe {:x}",
        page_va,
        match state {
            MemState::InMem => "InMem",
            MemState::Swapped => "Swapped",
            MemState::Exe => "Exe",
        },
        file_off.unwrap_or(0xbeef),
        tid,
        page_frame.unwrap_or(0xbeef)
    );*/
    unsafe {
        let mut glb_table = GLB_SWM.lock();

        if state == MemState::InMem && file_off.is_some() {
            let mut blockmap = &mut glb_table.block_map;
            blockmap.remove(&file_off.unwrap());
        } else if state == MemState::Swapped {
            let mut blockmap = &mut glb_table.block_map;
            blockmap.insert(file_off.unwrap(), page_va);
        }

        let mut frame_table = &mut glb_table.frame_table;
        if state == MemState::InMem {
            frame_table.push_back(FrameTableEntry {
                frame_va: page_frame.unwrap(),
                stored_va: page_va,
                tid,
                time: TIMESTAMP.fetch_add(1, core::sync::atomic::Ordering::SeqCst) as isize,
                busy: false,
                ref_cnt: ref_cnt,
            });
        } else if state == MemState::Swapped {
            frame_table.retain(|x| x.stored_va != page_va || x.tid != tid);
        }
    }
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

pub fn select_page() -> (usize, isize) {
    unsafe {
        let mut glbtb = GLB_SWM.lock();
        let frametb = &mut glbtb.frame_table;
        let mut pin_cnt = 0;

        // kprintln!("{}", frametb.len());

        let mut min_time = isize::MAX;
        let mut min_index = 0;

        for i in 0..frametb.len() {
            if frametb[i].busy {
                continue; // moving page
            }
            if frametb[i].ref_cnt > 0 {
                // kprintln!("found pin");
                pin_cnt += 1;
                continue; // pinned page
            }
            if frametb[i].time < min_time {
                min_time = frametb[i].time;
                min_index = i;
            }
        }

        // kprintln!("pin count: {}", pin_cnt);

        if min_time == isize::MAX {
            panic!("no available page to swap out");
        }

        if frametb.len() == 0 {
            kprintln!("called");
            return (1, 0);
        }

        frametb[min_index].busy = true;
        return (frametb[min_index].stored_va, frametb[min_index].tid);
    }
    panic!("select page error");
    /*
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

    panic!("select page error");*/
}
