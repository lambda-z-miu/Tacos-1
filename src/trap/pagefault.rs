use crate::mem::palloc::UserPool;
use crate::mem::userbuf::{
    __knrl_read_usr_byte_pc, __knrl_read_usr_exit, __knrl_write_usr_byte_pc, __knrl_write_usr_exit,
};
use crate::mem::{Entry, PTEFlags, PageTable, PhysAddr, PG_SIZE, VM_OFFSET};
use crate::thread::{self, current};
use crate::trap::{flags, Frame};
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
    /*
    kprintln!(
        "in pt : {}",
        current()
            .pagetable
            .as_ref()
            .unwrap()
            .lock()
            .get_pte(addr)
            .unwrap()
            .is_valid()
    );*/

    if !present {
        let current_sp = frame.x[2];
        kprintln!(
            "user stack base at {:x}, sp at {:x}, accessing {:x}",
            current().stack_base.unwrap_or(0xbeef),
            current_sp,
            addr
        );
        kprintln!("{}", current().stack_base.unwrap_or(0) - addr);
        if (current().stack_base.unwrap_or(0) - addr < MAX_STACK) && (addr > current_sp) {
            kprintln!("ALLOCK NEW pAGE");
            // panic!("log");
            alloc_from_pool(addr);
            return;
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

pub fn alloc_from_pool(addr: usize) {
    let va_alloc = unsafe { UserPool::alloc_pages(1) };
    let mut flag = PTEFlags::V;
    flag.set(PTEFlags::R, true);
    flag.set(PTEFlags::U, true);
    flag.set(PTEFlags::W, true);
    kprintln!("A");
    current().pagetable.as_ref().unwrap().lock().map(
        PhysAddr::from(va_alloc),
        addr - (addr % PG_SIZE),
        1,
        flag,
    );
}
