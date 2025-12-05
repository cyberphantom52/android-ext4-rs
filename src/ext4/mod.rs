mod block;
mod directory;
mod extent;
mod file;
mod inode;
mod superblock;
mod volume;
mod walker;

pub use block::BlockGroupDescriptor;
pub use extent::{Extent, ExtentHeader, ExtentIndex};
pub use file::File;
pub use inode::{Inode, Linux2};
pub use superblock::Superblock;
pub use volume::Volume;
pub use walker::{DirectoryWalker, WalkItem};

use thiserror::Error;

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

#[derive(Error, Debug)]
pub enum Ext4Error {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Parse error: {0}")]
    Parse(String),

    #[error("Invalid magic number")]
    InvalidMagic,

    #[error("Invalid superblock")]
    InvalidSuperblock,

    #[error("Invalid inode: {0}")]
    InvalidInode(u32),

    #[error("Invalid block group: {0}")]
    InvalidBlockGroup(u32),

    #[error("Invalid extent")]
    InvalidExtent,

    #[error("File not found: {0}")]
    FileNotFound(String),

    #[error("Not a directory")]
    NotADirectory,

    #[error("Invalid path: {0}")]
    InvalidPath(String),

    #[error("Read beyond file size")]
    ReadBeyondEof,
}

pub type Result<T> = std::result::Result<T, Ext4Error>;
