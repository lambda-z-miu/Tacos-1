use core::panic;
use core::slice::from_raw_parts;

use self::util::*;
use crate::mem::allocdata::AllocType;
use crate::mem::allocdata::PageInfo;
use crate::mem::palloc::UserPool;
use crate::mem::swapmanager::EXITLOCK;
use crate::mem::userbuf::{
    __knrl_read_usr_byte_pc, __knrl_read_usr_exit, __knrl_write_usr_byte_pc, __knrl_write_usr_exit,
};
use crate::mem::PG_MASK;
use crate::mem::{swapmanager, swapmem, Entry, PTEFlags, PageTable, PhysAddr, PG_SIZE, VM_OFFSET};
use crate::sync::Lock;
use crate::thread::schedule;
use crate::thread::{self, current};
use crate::trap::{flags, fscall, syscall, util, Frame};
use crate::userproc;

use riscv::register::scause::Exception::{self, *};
use riscv::register::sstatus::{self, SPP};

const MAX_STACK: usize = 0x800000;

pub fn handler(frame: &mut Frame, fault: Exception, addr: usize) {
    let privilege = frame.sstatus.spp();

    let mut table = unsafe { PageTable::effective_pagetable() };
    let present = {
        match table.get_pte(addr) {
            Some(entry) => entry.is_valid(),
            None => false,
        }
    };
    unsafe { /*
         kprintln!(
             "REPORT : Page fault at {:#x}:  error {} page , {} error, from thread {}",
             addr,
             match fault {
                 StorePageFault => "writing",
                 LoadPageFault => "reading",
                 InstructionPageFault => "fetching instruction",
                 _ => panic!("Unknown Page Fault"),
             },
             if present { "right" } else { "not present" },
             current().id()
         );*/
    }
    unsafe { sstatus::set_sie() };

    if !present {
        // EXITLOCK.acquire();
        let mut found_page = false;
        let mut need_wait = false;
        let addr_base = addr - (addr % PG_SIZE);
        loop {
            let thread = current();
            {
                let swap_table = thread.swap_table.lock();
                for i in swap_table.iter() {
                    if i.addr == addr_base {
                        // kprintln!("called loop");
                        found_page = true; // found the page in swap table
                        if i.state == swapmanager::MemState::InMem {
                            need_wait = true; // if it is in air, then wait
                        } else {
                            need_wait = false;
                        }
                        break;
                    }
                }
            }
            if (!need_wait) {
                break;
            } else {
                schedule();
            }
        }

        if found_page {
            loop {
                if (swapmem::swapin(addr_base)) {
                    break;
                }
                let mut victim = swapmanager::select_page();
                swapmem::swapout_pt(victim, None, None);
                unsafe {
                    riscv::asm::sfence_vma_all();

                    // 如果是指令页缺页，必须刷新 I-Cache
                    if fault == InstructionPageFault {
                        core::arch::asm!("fence.i");
                    }
                }
            }
            return;
            // assert!(victim.0 % PG_SIZE == 0);
            /*
            if victim.0 == 0x1000 {
                unsafe {
                    let thread = current();
                    let kva = thread
                        .pagetable
                        .as_ref()
                        .unwrap()
                        .lock()
                        .get_pte(0x1000)
                        .unwrap()
                        .pa()
                        .into_va();
                    let p = core::slice::from_raw_parts(kva as *const u8, 4096);
                    for i in p {
                        // kprint!("{:x} ", i);
                    }
                }
            }
            // kprintln!("chosen victim page at {:x} thread {}", victim.0, victim.1);

            swapmem::swapout_pt(victim, None, None);
            swapmem::swapin(addr_base);
            unsafe {
                riscv::asm::sfence_vma_all();

                // 如果是指令页缺页，必须刷新 I-Cache
                if fault == InstructionPageFault {
                    core::arch::asm!("fence.i");
                }
            }*/
        }
        // kprintln!("PAGE NOT FOUND");
        // EXITLOCK.release();
        let mut current_sp = frame.x[2];
        if current_sp > VM_OFFSET {
            // from user
            current_sp = current_sp - VM_OFFSET;
        }
        /*
        kprintln!(
            "user stack base at {:x}, sp at {:x}, accessing {:x}",
            current().stack_base.unwrap_or(0xbeef),
            current_sp,
            addr
        );
        kprintln!("{}", current().stack_base.unwrap_or(0) - addr);*/

        // growing stack
        /*
        kprintln!(
            "addr at {:x}, base at{:x}, sp at {:x}",
            addr,
            current().stack_base.unwrap_or(0),
            current_sp
        );*/
        let base = current().stack_base.unwrap_or(0);
        if (addr > current_sp && base < addr + MAX_STACK && addr < base) {
            // panic!("log");
            kprint!("growing stack to {:x}\n", addr);
            alloc_from_pool(addr);
            return;
        }

        // lazy allocating mmap region
        // kprintln!("0x{:x} needed", addr);
        for i in current().mmap_info.lock().iter() {
            if i.in_map(addr) {
                // kprintln!("ALLOCING PAGE at 0x{:x} MMAPID {}", i.addr, i.mmap_id);
                for j in 0..i.pages {
                    // kprintln!("ALLOCK NEW PAGE at {}", i.addr + j * PG_SIZE);
                    alloc_from_pool(i.addr + j * PG_SIZE);

                    let pos_mem = fscall::tell_handler(i.fd).unwrap_or(-1);
                    if pos_mem < 0 {
                        panic!("Tell failed in page fault handler");
                    }
                    fscall::read_handler(i.fd, (i.addr + j * PG_SIZE) as *mut u8, PG_SIZE);
                    fscall::seek_handler(i.fd, pos_mem as u32);

                    let thread = current();
                    let pt = thread.pagetable.as_ref().unwrap();
                    pt.lock().get_pte(i.addr + j * PG_SIZE).unwrap().set_clean();
                    // to track if it is modified later
                }
                return;
            }
        }
    }

    kprintln!(
        "Page fault at {:#x}: {} error {} page in {} context.",
        addr,
        if present { "rights" } else { "not present" },
        match fault {
            StorePageFault => "writing",
            LoadPageFault => "reading",
            InstructionPageFault => "fetching instruction",
            _ => panic!("Unknown Page Fault"),
        },
        match privilege {
            SPP::Supervisor => "kernel",
            SPP::User => "user",
        }
    );

    match privilege {
        SPP::Supervisor => {
            if frame.sepc == __knrl_read_usr_byte_pc as _ {
                // Failed to read user byte from kernel space when trap in pagefault
                frame.x[11] = 1; // set a1 to non-zero
                frame.sepc = __knrl_read_usr_exit as _;
            } else if frame.sepc == __knrl_write_usr_byte_pc as _ {
                // Failed to write user byte from kernel space when trap in pagefault
                frame.x[11] = 1; // set a1 to non-zero
                frame.sepc = __knrl_write_usr_exit as _;
            } else {
                panic!("Kernel page fault");
            }
        }
        SPP::User => {
            kprintln!(
                "User thread {} dying due to page fault.",
                thread::current().name()
            );
            userproc::exit(-1);
        }
    }
}
