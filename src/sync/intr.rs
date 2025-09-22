use core::cell::Cell;

use crate::{sbi, sync::Lock, thread::current};

/// A lock based on disabling timer interrupt.
///
/// The most basic way to synchornize between threads is to turn off/on timer
/// interrpt. However, it has relatively larger overhead sometimes, so please
/// use it only when necessary (especially when handling threads).
///
/// On acquisition, it turns off the timer and record the old timer status. On release,
/// it simply restores the old status.
#[derive(Debug, Default)]
pub struct Intr(Cell<Option<bool>>);

impl Intr {
    pub const fn new() -> Self {
        Self(Cell::new(None))
    }
}

unsafe impl Sync for Intr {}
unsafe impl Send for Intr {}

impl Lock for Intr {
    fn acquire(&self) {
        let old = crate::sbi::interrupt::set(false);
        if !self.0.get().is_none() {
            let debug = 1;
            kprintln!("debug : {}", !self.0.get().is_none());
            panic!("!!! in tread {}", current().name());
        }
        assert!(self.0.get().is_none());
        crate::sbi::interrupt::set(old);

        // Record the old timer status. Here setting the immutable `self` is safe
        // because the interrupt is already turned off.
        self.0.set(Some(sbi::interrupt::set(false)));
    }

    fn release(&self) {
        sbi::interrupt::set(self.0.take().expect("release before acquire"));
    }
}
