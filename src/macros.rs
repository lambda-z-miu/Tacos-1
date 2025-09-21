macro_rules! sbi_call {

    // v0.1
    ( $eid: expr; $($args: expr),* ) => { sbi_call!($eid, 0; $($args),*).0 };

    // v0.2
    ( $eid: expr, $fid: expr; $($arg0: expr $(, $arg1: expr )?)? ) => {
        {
            let (err, ret): (usize, usize);
            unsafe {
                core::arch::asm!("ecall",
                    in("a7") $eid, lateout("a0") err,
                    in("a6") $fid, lateout("a1") ret,

                  $(in("a0") $arg0, $(in("a1") $arg1)?)?
                );
            }

            (err, ret)
        }
    };
}

macro_rules! kprint {
    ($($arg:tt)*) => {{
        use core::fmt::Write;
        drop(write!($crate::sbi::Kout, $($arg)*));
    }};
}

macro_rules! kprintln {
    () => { kprint("\n") };
    ($($arg:tt)*) => {
        kprint!($($arg)*);
        kprint!("\n");
    };
}
