use core::panic::AssertUnwindSafe;

use crate::mem::{swapmanager, swapmem};
use crate::trap::flags;
use alloc::vec;
use elf_rs::{Elf, ElfFile, ProgramHeaderEntry, ProgramHeaderFlags, ProgramType};

use crate::fs::File;
use crate::io::prelude::*;
use crate::mem::allocdata::*;
use crate::mem::allocdata::*;
use crate::mem::pagetable::{PTEFlags, PageTable};
use crate::mem::palloc::UserPool;
use crate::mem::{div_round_up, PageAlign, PhysAddr, PG_MASK, PG_SIZE};
use crate::thread::current;
use crate::{OsError, Result};

#[derive(Debug, Clone, Copy)]
pub(super) struct ExecInfo {
    pub entry_point: usize,
    pub init_sp: usize,
}

/// Loads an executable file
///
/// ## Params
/// - `pagetable`: User's pagetable. We install the mapping to executable codes into it.
///
/// ## Return
/// On success, returns `Ok(usize, usize)`:
/// - arg0: the entry point of user program
/// - arg1: the initial sp of user program
pub(super) fn load_executable(file: &mut File, pagetable: &mut PageTable) -> Result<ExecInfo> {
    let exec_info = load_elf(file, pagetable)?;

    // Initialize user stack.
    init_user_stack(pagetable, exec_info.init_sp);

    // Forbid modifying executable file when running
    file.deny_write();

    Ok(exec_info)
}

/// Parses the specified executable file and loads segments
fn load_elf(file: &mut File, pagetable: &mut PageTable) -> Result<ExecInfo> {
    // Ensure cursor is at the beginning
    file.rewind()?;

    let len = file.len()?;
    let mut buf = vec![0u8; len];
    file.read(&mut buf)?;

    let elf = match Elf::from_bytes(&buf) {
        Ok(Elf::Elf64(elf)) => elf,
        Ok(Elf::Elf32(_)) | Err(_) => return Err(OsError::UnknownFormat),
    };

    // load each loadable segment into memory
    elf.program_header_iter()
        .filter(|p| p.ph_type() == ProgramType::LOAD)
        .for_each(|p| load_segment(&buf, &p, pagetable));

    Ok(ExecInfo {
        entry_point: elf.elf_header().entry_point() as _,
        init_sp: 0x80500000,
    })
}

/// Loads one segment and installs pagetable mappings
fn load_segment(filebuf: &[u8], phdr: &ProgramHeaderEntry, pagetable: &mut PageTable) {
    assert_eq!(phdr.ph_type(), ProgramType::LOAD);

    // Meaningful contents of this segment starts from `fileoff`.
    let fileoff = phdr.offset() as usize;
    // But we will read and install from `read_pos`.
    let mut readpos = fileoff & !PG_MASK;

    // Install flags.
    let mut leaf_flag = PTEFlags::V | PTEFlags::U | PTEFlags::R;
    if phdr.flags().contains(ProgramHeaderFlags::EXECUTE) {
        leaf_flag |= PTEFlags::X;
    }
    if phdr.flags().contains(ProgramHeaderFlags::WRITE) {
        leaf_flag |= PTEFlags::W;
    }

    // Install position: `ubase`.
    let ubase = (phdr.vaddr() as usize) & !PG_MASK;
    let pageoff = (phdr.vaddr() as usize) & PG_MASK;
    assert_eq!(fileoff & PG_MASK, pageoff);

    // How many pages need to be allocated
    let pages = div_round_up(pageoff + phdr.memsz() as usize, PG_SIZE);
    let mut readbytes = phdr.filesz() as usize + pageoff;

    // Allocate & map pages
    for p in 0..pages {
        // kprintln!("LOAD A PAGE {:?}", phdr);
        let mut buf;
        unsafe {
            buf = UserPool::alloc_pages(1);
            if buf.is_some() {
                // kprintln!("buf at {:x}", buf.unwrap() as usize);
            }
            if buf.is_none() {
                let addr = swapmanager::select_page();

                // add to kernel PT
                // let pte = pagetable.get_pte(addr).unwrap();
                //  pte.set_write();
                // let mut current_pt = unsafe { PageTable::effective_pagetable() };
                // current_pt.map(pte.pa(), addr as usize, 1, pte.flag());

                swapmem::swapout_pt(addr as *mut u8, pagetable);
                buf = UserPool::alloc_pages(1);
                if buf.is_some() {
                    // kprintln!("buf at {:x}", buf.unwrap() as usize);
                }
                // kprintln!("REALLOC SUCC {}", buf.unwrap() as usize);
            }
        }
        let buf = buf.expect("msg");

        let page = unsafe { (buf as *mut [u8; PG_SIZE]).as_mut().unwrap() };

        // Read `readsz` bytes, fill remaining bytes with 0.
        let readsz = readbytes.min(PG_SIZE);
        page[..readsz].copy_from_slice(&filebuf[readpos..readpos + readsz]);
        page[readsz..].fill(0);

        // The installed page will be freed when pagetable drops, which happens
        // when user process exits. No manual resource collect is required.
        let uaddr = ubase + p * PG_SIZE;
        pagetable.map(buf.into(), uaddr, 1, leaf_flag);

        swapmanager::register(uaddr, swapmanager::MemState::InMem, leaf_flag, None);

        let kva = pagetable.get_pte(uaddr).unwrap().pa().into_va();
        if uaddr == 0x1000 {
            unsafe {
                let p = core::slice::from_raw_parts(kva as *const u8, 4096);
                for i in p {
                    // kprint!("{:x} ", i);
                }
                kprintln!("LODED at {:X}", PhysAddr::from(buf).value());
            }
        }
        // kprintln!("Registered uaddr {:x}", uaddr);

        let thread = current();
        let mut pageinfo = thread.page_info.lock();
        pageinfo.push(PageInfo {
            va: uaddr,
            page_type: AllocType::Init,
        });

        readbytes -= readsz;
        readpos += readsz;
    }

    assert_eq!(readbytes, 0);
}

/// Initializes the user stack.
fn init_user_stack(pagetable: &mut PageTable, init_sp: usize) {
    assert!(init_sp % PG_SIZE == 0, "initial sp address misaligns");

    // Allocate a page from UserPool as user stack.
    let mut stack_va;
    unsafe {
        stack_va = UserPool::alloc_pages(1);
        if stack_va.is_none() {
            let addr = swapmanager::select_page();
            swapmem::swapout_pt(addr as *mut u8, pagetable);
            stack_va = UserPool::alloc_pages(1);
        }
    }

    let flags = PTEFlags::V | PTEFlags::R | PTEFlags::W | PTEFlags::U;

    let stack_va = stack_va.expect("succ after feeing a page");

    let stack_pa = PhysAddr::from(stack_va as usize);

    // Get the start address of stack page
    let stack_page_begin = PageAlign::floor(init_sp - 1);
    swapmanager::register(stack_page_begin, swapmanager::MemState::InMem, flags, None);

    let thread = current();
    let mut pageinfo = thread.page_info.lock();
    pageinfo.push(PageInfo {
        va: stack_page_begin as usize,
        page_type: AllocType::Stack,
    });

    // Install mapping
    pagetable.map(stack_pa, stack_page_begin, PG_SIZE, flags);

    #[cfg(feature = "debug")]
    kprintln!(
        "[USERPROC] User Stack Mapping: (k){:p} -> (u) {:#x}",
        stack_va,
        stack_page_begin
    );
}
