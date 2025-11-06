use self::util::*;
use crate::mem::allocdata::AllocType;
use crate::mem::allocdata::PageInfo;
use crate::mem::palloc::UserPool;
use crate::mem::userbuf::{
    __knrl_read_usr_byte_pc, __knrl_read_usr_exit, __knrl_write_usr_byte_pc, __knrl_write_usr_exit,
};
use crate::mem::{Entry, PTEFlags, PageTable, PhysAddr, PG_SIZE, VM_OFFSET};
use crate::thread::{self, current};
use crate::trap::{flags, fscall, syscall, util, Frame};
use crate::userproc;

use riscv::register::scause::Exception::{self, *};
use riscv::register::sstatus::{self, SPP};

const MAX_STACK: usize = 0x800000;

pub fn handler(frame: &mut Frame, fault: Exception, addr: usize) {
    let privilege = frame.sstatus.spp();

    let present = {
        let table = unsafe { PageTable::effective_pagetable() };
        match table.get_pte(addr) {
            Some(entry) => entry.is_valid(),
            None => false,
        }
    };

    unsafe { sstatus::set_sie() };

    if !present {
        let current_sp = frame.x[2];
        /*
        kprintln!(
            "user stack base at {:x}, sp at {:x}, accessing {:x}",
            current().stack_base.unwrap_or(0xbeef),
            current_sp,
            addr
        );
        kprintln!("{}", current().stack_base.unwrap_or(0) - addr);*/

        // growing stack
        if ((addr > current_sp) && current().stack_base.unwrap_or(0) - addr < MAX_STACK) {
            // panic!("log");
            alloc_from_pool(addr);
            return;
        }

        // lazy allocating mmap region
        kprintln!("{} needed", addr);
        for i in current().mmap_info.lock().iter() {
            if i.in_map(addr) {
                for j in 0..i.pages {
                    kprintln!("ALLOCK NEW PAGE at {}", i.addr + j * PG_SIZE);
                    alloc_from_pool(i.addr + j * PG_SIZE);
                    fscall::read_handler(i.fd, (i.addr + j * PG_SIZE) as *mut u8, PG_SIZE);
                    return;
                }
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
