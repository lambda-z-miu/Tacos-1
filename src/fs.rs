//! File System Interface
//!

pub mod disk;
pub mod inmem;

use alloc::sync::Arc;

use crate::io::{Read, Seek, Write};
use crate::sync::Mutex;
use crate::Result;
#[derive(Clone, Copy, PartialEq, Eq)]
enum FileType {
    File,
    Dir,
}

/* -------------------------------------------------------------------------- */
/*                                 File System                                */
/* -------------------------------------------------------------------------- */

/// File system interface.
///
/// A file system receives an `Identifier` type, which helps
/// the FS to locate specific files.
///
/// Typically a FS has only 1 instance during kernel running,
/// thus, this trait is designed to be [`Send`] and [`Sync`].
///
/// ## Examples
/// See [`inmem::MemFs`].
pub trait FileSys: Sync + Send + Sized {
    type Path;
    type Device;

    fn mount(device: Self::Device) -> Result<Self>;
    fn unmount(&self);

    fn open(&self, id: Self::Path) -> Result<File>;
    fn close(&self, file: File);
    fn create(&self, id: Self::Path) -> Result<File>;
    fn create_dir(&self, id: Self::Path) -> Result<File>;
    fn change_dir(&self, id: Self::Path) -> Result<()>;
    fn remove(&self, id: Self::Path) -> Result<()>;
}

/* -------------------------------------------------------------------------- */
/*                                Virtual Inode                               */
/* -------------------------------------------------------------------------- */

/// Virtual inode interface.
///
/// An inode is typically held by one or multiple [`File`]
/// and provides methods to allow [`File`]s access the data.
///
/// Typically an inode can be referenced by multiple [`File`]s,
/// thus, this trait is designed to be [`Send`] and [`Sync`].
pub trait Vnode: Sync + Send {
    fn read_at(&self, buf: &mut [u8], off: usize) -> Result<usize>;
    fn write_at(&self, buf: &[u8], off: usize) -> Result<usize>;
    fn deny_write(&self);
    fn allow_write(&self);

    fn inum(&self) -> usize;
    fn len(&self) -> usize;
    fn resize(&self, size: usize) -> Result<()>;
    fn close(&self);
}

/* -------------------------------------------------------------------------- */
/*                                    File                                    */
/* -------------------------------------------------------------------------- */

/// A file descriptor, binding with a [`Vnode`], that has
/// independent position and permissions. It provides basic
/// file I/O interface.
#[derive(Clone)]
pub struct File {
    vnode: Arc<dyn Vnode>,
    pos: usize,
    deny_write: bool,
    filetype: FileType,
}

impl File {
    pub fn set_len(&mut self, size: usize) -> Result<()> {
        self.vnode.resize(size)
    }

    pub fn inum(&self) -> usize {
        self.vnode.inum()
    }

    pub fn print(&mut self, items: usize) {
        kprintln!(
            "File inum: {}, len: {}",
            self.vnode.inum(),
            self.vnode.len()
        );
        if self.filetype == FileType::Dir {
            self.rewind().unwrap();
            for i in 0..items {
                let buf = &mut [0u8; 32];
                self.read(buf).unwrap();
                // kprintln!("This is a directory, {} entry raw data: {:?}", i, buf);
            }
        } else {
            kprintln!("This is a file.");
        }
    }

    pub fn is_dir(&self) -> bool {
        self.filetype == FileType::Dir
    }
}

impl Read for File {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
        let cnt = self.vnode.read_at(buf, self.pos)?;
        self.pos += cnt;
        Ok(cnt)
    }
}

impl Write for File {
    fn write(&mut self, buf: &[u8]) -> Result<usize> {
        let cnt = self.vnode.write_at(buf, self.pos)?;
        self.pos += cnt;
        Ok(cnt)
    }

    fn flush(&mut self) -> Result<()> {
        Ok(())
    }
}

impl Seek for File {
    fn len(&self) -> Result<usize> {
        Ok(self.vnode.len())
    }

    fn pos(&mut self) -> Result<&mut usize> {
        Ok(&mut self.pos)
    }
}

impl File {
    pub fn fstat(&self) -> (usize, usize) {
        (self.vnode.inum(), self.vnode.len())
    }

    pub fn set_pos(&mut self, set_pos: u32) {
        self.pos = set_pos as usize;
    }

    pub fn new(vnode: Arc<dyn Vnode>, filetype: FileType) -> Self {
        // kprintln!("is dir? {}", filetype == FileType::Dir);
        Self {
            vnode,
            pos: 0,
            deny_write: false,
            filetype: filetype,
        }
    }

    pub fn deny_write(&mut self) {
        self.deny_write = true;
        self.vnode.deny_write();
    }

    pub fn allow_write(&self) {
        self.vnode.allow_write();
    }
}

impl Drop for File {
    fn drop(&mut self) {
        if self.deny_write {
            self.vnode.allow_write();
        }
    }
}
