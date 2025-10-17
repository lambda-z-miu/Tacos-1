use crate::OsError;

const O_CREATE: usize = 0x200;
const O_TRUNC: usize = 0x400;
const WRITE_MASK: usize = 0b10;
const READ_MASK: usize = 0b1;

pub struct FdFlags {
    pub flag: usize,
}

impl FdFlags {
    pub fn write_permision(&self) -> Result<(), OsError> {
        if (self.flag + 1) & WRITE_MASK == WRITE_MASK {
            return Ok(());
        }
        return Err(OsError::PermissionDenied);
    }

    pub fn read_permision(&self) -> Result<(), OsError> {
        if (self.flag + 1) & READ_MASK == READ_MASK {
            return Ok(());
        }
        return Err(OsError::PermissionDenied);
    }

    pub fn get_create(&self) -> Result<(), OsError> {
        if (self.flag & O_CREATE) != 0 {
            return Ok(());
        }
        return Err(OsError::PermissionDenied);
    }
}
