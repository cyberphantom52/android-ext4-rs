mod block;
mod extent;
mod inode;
mod reader;
mod superblock;

pub use block::BlockGroupDescriptor;
pub use extent::{Extent, ExtentHeader, ExtentIndex};
pub use inode::{Inode, InodeFileType, InodePerm, Linux2};
pub use reader::Ext4Reader;
pub use superblock::Superblock;

use thiserror::Error;

pub const BLOCK_SIZE: usize = 0x1000;
pub const SECTORS_PER_BLOCK: usize = BLOCK_SIZE / 512;

pub type Ext4Lblk = u32;
pub type Ext4Fsblk = u64;

pub const EXT4_MAX_FILE_SIZE: u64 = 16 * 1024 * 1024 * 1024;

pub const O_ACCMODE: i32 = 0o0003;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum OpenFlags {
    ReadOnly = 0,
    WriteOnly = 1,
    ReadWrite = 2,
    Create = 0o100,
    Exclusive = 0o200,
    NoCTTY = 0o400,
    Truncate = 0o1000,
    Append = 0o2000,
    NonBlocking = 0o4000,
    Sync = 0o4010000,
    Async = 0o20000,
    LargeFile = 0o100000,
    Directory = 0o200000,
    NoFollow = 0o400000,
    CloExec = 0o2000000,
    Direct = 0o40000,
    NoAtime = 0o1000000,
    Path = 0o10000000,
    DSync = 0o10000,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum AccessMode {
    Exist = 0b000,
    Execute = 0b001,
    Write = 0b010,
    Read = 0b100,
}

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

impl DirEntryType {
    pub fn from_u8(value: u8) -> Self {
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
    pub fn name_str(&self) -> &str {
        let len = self.name_len as usize;
        std::str::from_utf8(&self.name[..len]).unwrap_or("")
    }

    pub fn entry_type(&self) -> DirEntryType {
        DirEntryType::from_u8(self.inode_type)
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
