use crate::mem::{PageTable, PhysAddr};
use crate::sbi::console::Stdout;
use crate::sbi::interrupt;
use crate::sync::Lock;

use alloc::slice;

use crate::fs::disk::Swap;
use crate::io::{Read, Write};
use crate::mem::palloc::{Palloc, UserPool};
use crate::mem::{swapmanager::*, PTEFlags, PG_SIZE};
use crate::thread::{self, current, manager, Status};
use crate::trap::fscall;
use alloc::collections::VecDeque;
use core::slice::{from_raw_parts, from_raw_parts_mut};
use core::sync::atomic::AtomicBool;
use core::{fmt, panic};

pub static mut REACHED: AtomicBool = AtomicBool::new(false);

pub fn swapin(inpage: usize) -> bool {
    assert!(inpage as usize % PG_SIZE == 0);
    let mut pos;
    let mut flags;
    let mut addr;

    unsafe {
        addr = UserPool::alloc_pages(1);
        if addr.is_none() {
            return false;
        }
    }

    let addr = addr.unwrap();

    {
        pos = get_page_pos(inpage).expect("swap in page not in swap file");
        flags = get_page_flags(inpage);
        assert!(get_page_state(inpage) == MemState::Swapped);

        // regist pt -> GET PT LOCK
        let thread = current();
        let mut pt = thread.pagetable.as_ref().unwrap().lock();
        pt.map(PhysAddr::from(addr as usize), inpage, 1, flags);
        unsafe {
            riscv::asm::sfence_vma_all();
        }
        unsafe {
            if REACHED.load(core::sync::atomic::Ordering::SeqCst) {
                // kprintln!("va {}");
            }
        }

        // regist global manager -> GET MNG LOCK
    }

    {
        // allocate a page, then read the page from swap file
        let mut swapfile = Swap::lock();
        swapfile.set_pos(pos);
        unsafe {
            assert!(addr as usize % PG_SIZE == 0);
            // kprintln!("REACHED3 in thread {}", current().id());
            if (swapfile
                .read(from_raw_parts_mut(addr, PG_SIZE))
                .expect("error when reading swap file")
                != PG_SIZE)
            {
                panic!("error when reading swap file");
            }
        }
    }
    {
        let thread = current();
        let mut swap_table = thread.swap_table.lock();
        clean_ste(&mut swap_table, inpage);
        register_swaptable(&mut swap_table, inpage, MemState::InMem, flags, None);
        // GET GLB LOCK
        register(
            inpage,
            current().id(),
            MemState::InMem,
            flags,
            Some(pos),
            Some(addr as usize),
        );
    }
    return true;
}

pub fn swapout_pt(
    outpage: (usize, isize),
    pt: Option<&mut PageTable>,
    swaptable: Option<&mut VecDeque<SwapTableEntry>>,
) {
    // assert!(outpage.0 % PG_SIZE == 0);
    if outpage.0 == 1 {
        kprintln!("called");
        unsafe {
            REACHED.store(true, core::sync::atomic::Ordering::SeqCst);
        }
        // panic!("cannot swap out page 0");
        return;
    }

    // get PTE, PA, kernel VA

    let mut found = false;
    let mut kva;
    let pos = get_slot();

    let thread = manager::Manager::get_by_tid(outpage.1);

    if let Some(i) = thread {
        // operating a thread that already exists
        found = true;
        let mut pte_flag;
        {
            // get pt -> GET PT LOCK

            let pagetable = i.pagetable.as_ref().unwrap();
            let pt = pagetable.lock();
            let pte = pt.get_pte(outpage.0).unwrap();
            // extract info, invalidate
            pte_flag = pte.flag();
            kva = pte.pa().into_va();
            let pte = pt.get_pte(outpage.0).expect("pte should exist");
            pte.set_invalid();
            unsafe {
                riscv::asm::sfence_vma_all();
            }
        }

        {
            let mut swapfile = Swap::lock();
            swapfile.set_pos(pos);
            unsafe {
                assert!(kva as usize % PG_SIZE == 0);
                if (swapfile
                    .write(from_raw_parts(kva as *const u8, PG_SIZE))
                    .expect("error when writing swap file")
                    != PG_SIZE)
                {
                    panic!("error when writing swap file");
                }
            }
        }

        unsafe {
            UserPool::dealloc_pages(kva as *mut u8, 1);
        }

        {
            // get info from swaptable -> GET SWATTLB LOCK
            let mut swap_table = i.swap_table.lock();
            clean_ste(&mut swap_table, outpage.0 as usize);
            register_swaptable(
                &mut swap_table,
                outpage.0,
                MemState::Swapped,
                pte_flag,
                Some(pos),
            );

            // get GLB -> GET MNG LOCK
            register(
                outpage.0,
                outpage.1,
                MemState::Swapped,
                pte_flag,
                Some(pos),
                None,
            );
        }
    } else {
        if pt.is_none() {
            /*
            kprintln!(
                "SWAPOUT_PT: thread {} does not exist, current in {}",
                outpage.1,
                current().id()
            );*/
            return;
            panic!("cannot get page table");
        }

        let pt = pt.unwrap();

        // invalidate, extract info
        let pte = pt.get_pte(outpage.0).expect("pte should exist");
        kva = pte.pa().into_va();
        let pte_flag = pte.flag();
        pte.set_invalid();

        let mut swapfile = Swap::lock();
        swapfile.set_pos(pos);
        unsafe {
            assert!(kva as usize % PG_SIZE == 0);
            if (swapfile
                .write(from_raw_parts(kva as *const u8, PG_SIZE))
                .expect("error when writing swap file")
                != PG_SIZE)
            {
                panic!("error when writing swap file");
            }
        }

        unsafe {
            UserPool::dealloc_pages(kva as *mut u8, 1);
        }

        // regist at swaptable
        // MNGLOCK.acquire();
        let mut swaptable = swaptable.unwrap();
        clean_ste(&mut swaptable, outpage.0);
        register_swaptable(
            &mut swaptable,
            outpage.0,
            MemState::Swapped,
            pte_flag,
            Some(pos),
        );

        // regist global info
        register(
            outpage.0,
            outpage.1,
            MemState::Swapped,
            pte_flag,
            Some(pos),
            None,
        );
        // MNGLOCK.release();
    }

    // finding a slot, write to swap file
    // kprintln!("L130 REACHED By thread {}", current().id());

    /*

        unsafe {
            riscv::asm::sfence_vma_all();
        }

        // invalidate kernel and user PTE
        // pte.set_invalid();
        unsafe {
            riscv::asm::sfence_vma_all();
        }

        unsafe {
            UserPool::dealloc_pages(kva as *mut u8, 1);
            riscv::asm::sfence_vma_all();
        }
    */
}
