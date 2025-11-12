use crate::mem::{PageTable, PhysAddr};
use crate::sbi::interrupt;

use alloc::slice;

use crate::fs::disk::Swap;
use crate::io::{Read, Write};
use crate::mem::palloc::{Palloc, UserPool};
use crate::mem::{swapmanager::*, PTEFlags, PG_SIZE};
use crate::thread::{self, current};
use crate::trap::fscall;
use core::slice::{from_raw_parts, from_raw_parts_mut};
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
        if (swapfile
            .read(from_raw_parts_mut(addr, PG_SIZE))
            .expect("error when reading swap file")
            != PG_SIZE)
        {
            panic!("error when reading swap file");
        }

        unsafe {
            let after = *((addr as usize) as *const u8);
            // kprintln!("  Buffer byte[0] after read: {:#x}", after);
        }
        // register swaptable
        clean_ste(inpage);
        register(inpage, MemState::InMem, flags, Some(pos));

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

pub fn swapout_pt(outpage: *mut u8, pt: &mut PageTable) {
    assert!(outpage as usize % PG_SIZE == 0);
    if outpage as usize == 0x1000 {
        kprintln!("SWAPOUT");
    }
    let tmp = interrupt::set(false);

    // get PTE, PA, kernel VA
    let mut pte = pt.get_pte(outpage as usize).expect("not in mem");
    let pa = pte.pa().value();
    let kva = pa + crate::mem::layout::VM_OFFSET;

    unsafe {
        let first_16 = core::slice::from_raw_parts(kva as *const u8, 16);

        let byte_4080 = *((kva) as *const u8);
    }

    // finding a slot
    let pos = get_slot();

    // write to swap file, then dealloc page

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

    // regist at swaptable
    clean_ste(outpage as usize);
    register(outpage as usize, MemState::Swapped, pte.flag(), Some(pos));

    unsafe {
        riscv::asm::sfence_vma_all();
    }

    // invalidate kernel and user PTE
    pte.set_invalid();
    unsafe {
        riscv::asm::sfence_vma_all();
    }

    unsafe {
        UserPool::dealloc_pages(kva as *mut u8, 1);
        riscv::asm::sfence_vma_all();
    }

    interrupt::set(tmp);
}
