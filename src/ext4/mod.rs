mod block;
mod directory;
mod extent;
mod file;
mod inode;
mod inode_reader;
mod superblock;
mod volume;
mod walker;
mod xattr;

pub use directory::Directory;
pub use file::File;
pub use inode::FileType;
use inode_reader::InodeReader;
pub use volume::Volume;
pub use walker::{DirectoryWalker, EntryAttributes, WalkItem};

// Re-export errors from utils
pub use crate::utils::{Error, ParseContext, Result};

pub type Ext4Lblk = u32;
pub type Ext4Fsblk = u64;
pub const ADDR_SIZE: u32 = 4;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum DirEntryType {
    Unknown = 0,
    RegFile = 1,
    Dir = 2,
    ChrDev = 3,
    BlkDev = 4,
    Fifo = 5,
    Sock = 6,
    Symlink = 7,
}

impl From<u8> for DirEntryType {
    fn from(value: u8) -> Self {
        match value {
            0 => Self::Unknown,
            1 => Self::RegFile,
            2 => Self::Dir,
            3 => Self::ChrDev,
            4 => Self::BlkDev,
            5 => Self::Fifo,
            6 => Self::Sock,
            7 => Self::Symlink,
            _ => Self::Unknown,
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct DirectoryEntry {
    pub inode: u32,
    pub entry_len: u16,
    pub name_len: u8,
    pub inode_type: u8,
    pub name: [u8; 255],
}

impl DirectoryEntry {
    pub const HEADER_SIZE: usize = 8;
    pub const MAX_NAME_LEN: usize = 255;
    pub fn name_str(&self) -> &str {
        let len = self.name_len as usize;
        std::str::from_utf8(&self.name[..len]).unwrap_or("")
    }

    pub fn entry_type(&self) -> DirEntryType {
        DirEntryType::from(self.inode_type)
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct DirectoryEntryTail {
    pub reserved_zero1: u32,
    pub rec_len: u16,
    pub reserved_zero2: u8,
    pub reserved_ft: u8,
    pub checksum: u32,
}

pub struct DirectorySearchResult {
    pub dentry: DirectoryEntry,
    pub pblock_id: u64,
    pub offset: usize,
    pub prev_offset: usize,
}
