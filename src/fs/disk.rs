//! On disk file system.
//!
mod dir;
mod free_map;
mod inode;
mod path;
mod swap;

use core::char::MAX;
use core::convert::TryInto;
use core::mem::size_of;
use core::slice::{self, from_raw_parts};
use core::u32;

// Expose path for it is frequently used.
pub use self::path::Path;
// Expose swap utils.
pub use self::swap::Swap;

use alloc::collections::BTreeMap;
use alloc::sync::{Arc, Weak};

use self::dir::Dir;
use self::dir::FILE_NAME_LEN_MAX;
use self::free_map::FreeMap;
use self::inode::Inode;

use super::{File, FileSys, Vnode};
use crate::device::virtio::{Virtio, SECTOR_SIZE};
use crate::fs::disk::dir::DirEntry;
use crate::fs::FileType;
use crate::io::Write;
use crate::sync::{Lazy, Mutex};
use crate::{OsError, Result};

/// Inode number.
///  
/// We use sector number equivalently as inode number.
pub type Inum = u32;

/// Inumber of sector free bitmap.
pub(self) const FREE_MAP_SECTOR: Inum = 0;

/// Inumber of root dir.
pub(self) const ROOT_DIR_SECTOR: Inum = 1;

/// Root dir length in sector.
///
/// Currently we hard code this.
const ROOT_DIR_SECTOR_LEN: u32 = 8;

/// Global disk filesys.
///
/// # Usage
///
/// - **get disk sector free bitmap:**
/// ```ignore
/// let freemap = DISKFS.free_map.lock();
/// // Do sth.
/// let new_sector = freemap.alloc(1);
/// ```
///
/// - **get root dir:**
/// ```ignore
/// let rootdir = DISKFS.root_dir.lock();
/// // Do sth.
/// let if_exist = rootdir.exists("/myfile".into());
/// ```
///
/// - **file operations (create, open, remove):**
/// ```ignore
/// // create
/// let file = DISKFS.create("/new_file".into(), 18)?;
/// file.write_all([1u8; 18]);
/// // open
/// let file2 = DISKFS.open("/new_file".into())?;
/// let mut buf = [0u8; 18];
/// file2.read_exact(&mut buf);
/// // remove
/// DISKFS.remove("/new_file".into())?;
/// ```
pub static DISKFS: Lazy<DiskFs> =
    Lazy::new(|| DiskFs::mount(Virtio::get()).expect("Disk fs mounting failed"));

/// Disk file system.
///
/// # See
/// [`crate::fs::disk::DISKFS`].
pub struct DiskFs {
    #[allow(unused)]
    device: &'static Mutex<Virtio>,
    pub(self) free_map: Mutex<FreeMap>,
    pub root_dir: Mutex<Dir>,
    pub current_dir: Mutex<Dir>,
    inode_table: Mutex<BTreeMap<Inum, Weak<Inode>>>,
}

impl FileSys for DiskFs {
    type Device = &'static Mutex<Virtio>;
    type Path = Path;

    fn mount(device: Self::Device) -> Result<Self> {
        let capacity = device.lock().capacity();
        let inode_table = Mutex::new(BTreeMap::new());
        let free_map = Mutex::new({
            let size = capacity as u32;
            if let Ok(loaded) = FreeMap::load(size) {
                loaded
            } else {
                FreeMap::new_format(size)?
            }
        });
        let inner_dir = {
            let vnode = if let Ok(loaded) = Inode::open(ROOT_DIR_SECTOR) {
                loaded
            } else {
                let start = free_map.lock().alloc(ROOT_DIR_SECTOR_LEN)?;

                #[cfg(feature = "debug")]
                kprintln!(
                    "Rootdir format at sector {}, len={}",
                    start,
                    ROOT_DIR_SECTOR_LEN
                );

                Inode::create(
                    ROOT_DIR_SECTOR,
                    start,
                    ROOT_DIR_SECTOR_LEN as usize * SECTOR_SIZE,
                )?
            };

            let weak = Arc::downgrade(&vnode);
            inode_table.lock().insert(ROOT_DIR_SECTOR, weak);
            let mut inner_dir = Dir(File::new(vnode, FileType::Dir));
            DiskFs::init_dir(&mut inner_dir.0, ROOT_DIR_SECTOR);
            inner_dir
        };
        Ok(Self {
            device,
            free_map,
            root_dir: Mutex::new(inner_dir.clone()),
            current_dir: Mutex::new(inner_dir.clone()),
            inode_table,
        })
    }

    fn unmount(&self) {
        let _ = self.free_map.lock().flush();
    }

    fn create(&self, id: Self::Path) -> Result<super::File> {
        let vnode = if self.current_dir.lock().exists(&id) {
            let inum = self.current_dir.lock().path2inum(&id).unwrap();
            let vnode =
                if let Some(arc) = self.inode_table.lock().get(&inum).and_then(Weak::upgrade) {
                    arc
                } else {
                    Inode::open(inum)?
                };
            // Trunc existing file to 0 on create.
            vnode.resize(0)?;
            vnode
        } else {
            let sector = self.free_map.lock().alloc(1)?;

            let cnt = bytes_to_sectors(0);
            let start = self.free_map.lock().alloc(cnt)?;

            let vnode = Inode::create(sector, start, 0)?;
            let weak = Arc::downgrade(&vnode);
            self.inode_table.lock().insert(sector, weak);

            self.current_dir.lock().insert(&id, sector)?;
            kprintln!("Inserted entry '{}' in current dir, now", id.as_str());
            self.current_dir.lock().0.print(3);
            vnode
        };

        Ok(File::new(vnode, FileType::File))
    }

    fn create_dir(&self, id: Path) -> Result<File> {
        let vnode = if self.current_dir.lock().exists(&id) {
            return Err(OsError::FileExists);
        } else {
            let sector = self.free_map.lock().alloc(1)?;

            let cnt = bytes_to_sectors(0);
            let start = self.free_map.lock().alloc(cnt)?;

            let vnode = Inode::create(sector, start, 0)?;
            let weak = Arc::downgrade(&vnode);
            self.inode_table.lock().insert(sector, weak);

            self.current_dir.lock().insert(&id, sector)?;
            vnode
        };
        let mut dir = File::new(vnode, FileType::Dir);
        DiskFs::init_dir(&mut dir, self.current_dir.lock().0.inum() as u32)?;
        kprintln!("Created directory '{}'", dir.inum());
        // dir.print(2);
        Ok(dir)
    }

    fn open(&self, id: Self::Path) -> Result<super::File> {
        if !self.current_dir.lock().exists(&id) {
            return Err(OsError::NoSuchFile);
        }
        kprintln!("Found");
        // Expect existing.
        let inum = self.current_dir.lock().path2inum(&id).unwrap();
        if let Some(arc) = self.inode_table.lock().get(&inum).and_then(Weak::upgrade) {
            return Ok(File::new(arc, FileType::File));
        }

        let vnode = Inode::open(inum)?;
        kprintln!("Opened file inum {}", inum);
        let weak = Arc::downgrade(&vnode);
        self.inode_table.lock().insert(inum, weak);

        Ok(File::new(vnode, FileType::File))
    }

    fn close(&self, file: super::File) {
        file.vnode.close();
    }

    fn remove(&self, id: Self::Path) -> Result<()> {
        let inum = self.current_dir.lock().path2inum(&id)?;
        let mut current_dir = self.current_dir.lock();
        current_dir.remove(inum)?;
        if let Some(arc) = self.inode_table.lock().get(&inum).and_then(Weak::upgrade) {
            arc.remove();
            return Ok(());
        }
        // Not opened
        let inode = Inode::open(inum)?;
        inode.remove();
        Ok(())
    }

    fn change_dir(&self, id: Self::Path) -> Result<()> {
        if !self.current_dir.lock().exists(&id) {
            return Err(OsError::NoSuchFile);
        }
        let inum = self.current_dir.lock().path2inum(&id).unwrap();
        let vnode = if let Some(arc) = self.inode_table.lock().get(&inum).and_then(Weak::upgrade) {
            arc
        } else {
            Inode::open(inum)?
        };
        let mut dir = Dir(File::new(vnode, FileType::Dir));
        // dir.0.print(2);
        *self.current_dir.lock() = dir;
        kprintln!(
            "Changed current dir to '{}'",
            self.current_dir.lock().0.inum()
        );
        Ok(())
    }
}

impl DiskFs {
    fn init_dir(dir: &mut File, inode_parent: u32) -> Result<()> {
        assert!(dir.filetype == FileType::Dir);
        let dot = DirEntry {
            name: {
                let mut arr = [0u8; FILE_NAME_LEN_MAX];
                arr[0] = b'.';
                arr
            },
            inum: dir.inum() as u32,
        };
        let dotdot = DirEntry {
            name: {
                let mut arr = [0u8; FILE_NAME_LEN_MAX];
                arr[0] = b'.';
                arr[1] = b'.';
                arr
            },
            inum: inode_parent,
        };
        let size = size_of::<DirEntry>();
        unsafe {
            dir.write(from_raw_parts(
                &dot as *const DirEntry as *const u8,
                FILE_NAME_LEN_MAX + 4,
            ))?;
            dir.write(from_raw_parts(
                &dotdot as *const DirEntry as *const u8,
                FILE_NAME_LEN_MAX + 4,
            ))?;
        }
        Ok(())
    }
}

pub(self) fn bytes_to_sectors(bytes: usize) -> u32 {
    ((bytes + SECTOR_SIZE - 1) / SECTOR_SIZE) as u32
}
