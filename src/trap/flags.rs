const O_CREATE: usize = 0x200;
const O_TRUNC: usize = 0x400;

pub struct FdFlags {
    pub flag: usize,
}

impl FdFlags {
    pub fn write_permision(&self) -> bool {
        (self.flag + 1) & 0b10 == 0b10
    }

    pub fn read_permision(&self) -> bool {
        (self.flag + 1) & 0b1 == 0b01
    }

    pub fn get_create(&self) -> bool {
        (self.flag & O_CREATE) != 0
    }
}
