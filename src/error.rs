//! Os Error Handling.
//!

/// Possible errors in the OS
#[repr(i32)]
#[derive(Debug, PartialEq)]
pub enum OsError {
    BadPtr = -1,
    UnexpectedEOF = -2,
    NoSuchFile = -3,
    UnknownFormat = -4,
    UserError = -5,
    CreateExistInode = -6,
    OpenInvalidInode = -7,
    DiskSectorAllocFail = -8,
    RootDirFull = -9,
    CstrFormatErr = -10,
    ArgumentTooLong = -11,
    InvalidFileMode = -12,
    FileNotOpened = -13,
    UnAlignedAccess = -14,
    FileNotExist = -15,
    PermissionDenied = -16,
    OverlappingMMap = -17,
    MMapIDNotExist = -18,
    NotADirectory = -19,
    FileExists = -20,
    DirNotEmpty = -21,
}
