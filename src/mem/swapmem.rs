use crate::mem::{PageTable, PhysAddr};
use crate::sbi::interrupt;
use crate::sync::Lock;

use alloc::slice;

use crate::fs::disk::Swap;
use crate::io::{Read, Write};
use crate::mem::palloc::{Palloc, UserPool};
use crate::mem::{swapmanager::*, PTEFlags, PG_SIZE};
use crate::thread::{self, current, manager};
use crate::trap::fscall;
use alloc::collections::VecDeque;
use core::slice::{from_raw_parts, from_raw_parts_mut};
use core::{fmt, panic};
pub fn swapin(inpage: usize) {
    assert!(inpage as usize % PG_SIZE == 0);
    // get info from swaptable
    let pos = get_page_pos(inpage).expect("swap in page not in swap file");
    let flags = get_page_flags(inpage);
    assert!(get_page_state(inpage) == MemState::Swapped);

    // allocate a page, then read the page from swap file
    // kprintln!("SWAPFILELOCK ACC BY SWAPIN");
    let mut swapfile = Swap::lock();
    swapfile.set_pos(pos);
    unsafe {
        let addr = UserPool::alloc_pages(1).expect("a page should have been evicted");
        assert!(addr as usize % PG_SIZE == 0);
        // kprintln!("REACHED3 in thread {}", current().id());
        if (swapfile
            .read(from_raw_parts_mut(addr, PG_SIZE))
            .expect("error when reading swap file")
            != PG_SIZE)
        {
            panic!("error when reading swap file");
        }

        // register swaptable
        let thread = current();
        let mut swap_table = thread.swap_table.lock();

        // MNGLOCK.acquire();
        clean_ste(&mut swap_table, inpage);
        register_swaptable(&mut swap_table, inpage, MemState::InMem, flags, None);

        // register global info
        register(
            inpage,
            current().id(),
            MemState::InMem,
            flags,
            Some(pos),
            Some(addr as usize),
        );
        // MNGLOCK.release();

        // map in pt
        let thread = current();
        let mut pt = thread.pagetable.as_ref().unwrap().lock();
        unsafe {
            riscv::asm::sfence_vma_all();
        }
        pt.map(PhysAddr::from(addr as usize), inpage, 1, flags);
        unsafe {
            riscv::asm::sfence_vma_all();
        }
    }
    // kprintln!("SWAPFILELOCK REL BY SWAPIN");
}

pub fn swapout_pt(
    outpage: (usize, isize),
    pt: Option<&mut PageTable>,
    swaptable: Option<&mut VecDeque<SwapTableEntry>>,
) {
    assert!(outpage.0 % PG_SIZE == 0);

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
            // get PTE
            let pagetable = i.pagetable.as_ref().unwrap();
            let pt = pagetable.lock();
            let pte = pt.get_pte(outpage.0);
            let pte = pte.unwrap();

            // extract info, invalidate
            pte_flag = pte.flag();
            kva = pte.pa().into_va();
            pte.set_invalid();
            unsafe {
                riscv::asm::sfence_vma_all();
            }
        }
        {
            let mut swap_table = i.swap_table.lock();
            clean_ste(&mut swap_table, outpage.0 as usize);
            register_swaptable(
                &mut swap_table,
                outpage.0,
                MemState::Swapped,
                pte_flag,
                Some(pos),
            );
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

        // regist at swaptable
        // MNGLOCK.acquire();

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

        unsafe {
            UserPool::dealloc_pages(kva as *mut u8, 1);
        }
    } else {
        if pt.is_none() {
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
