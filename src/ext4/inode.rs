use bitflags::bitflags;
use nom::Finish;
use nom_derive::{NomLE, Parse};

use crate::{Ext4Error, Result};

#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, NomLE)]
pub struct Linux2 {
    pub l_i_blocks_high: u16,
    pub l_i_file_acl_high: u16,
    pub l_i_uid_high: u16,
    pub l_i_gid_high: u16,
    pub l_i_checksum_lo: u16,
    pub l_i_reserved: u16,
}

impl Linux2 {
    pub fn parse(bytes: &[u8]) -> Result<Self> {
        match Parse::parse(bytes).finish() {
            Ok((_, descriptor)) => Ok(descriptor),
            Err(e) => Err(Ext4Error::Parse(format!("{:?}", e))),
        }
    }
}

bitflags! {
    #[derive(Debug, PartialEq, Eq, Clone, Copy)]
    pub struct InodeFileType: u16 {
        const S_IFIFO = 0x1000;
        const S_IFCHR = 0x2000;
        const S_IFDIR = 0x4000;
        const S_IFBLK = 0x6000;
        const S_IFREG = 0x8000;
        const S_IFSOCK = 0xC000;
        const S_IFLNK = 0xA000;
    }
}

bitflags! {
    #[derive(Debug, PartialEq, Eq, Clone, Copy)]
    pub struct InodePerm: u16 {
        const S_IREAD = 0x0100;
        const S_IWRITE = 0x0080;
        const S_IEXEC = 0x0040;
        const S_ISUID = 0x0800;
        const S_ISGID = 0x0400;
    }
}

#[repr(C)]
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, NomLE)]
pub struct Inode {
    pub mode: u16,
    pub uid: u16,
    pub size: u32,
    pub atime: u32,
    pub ctime: u32,
    pub mtime: u32,
    pub dtime: u32,
    pub gid: u16,
    pub links_count: u16,
    pub blocks: u32,
    pub flags: u32,
    pub osd1: u32,
    pub block: [u32; 15],
    pub generation: u32,
    pub file_acl: u32,
    pub size_hi: u32,
    pub faddr: u32,
    pub osd2: Linux2,
    pub i_extra_isize: u16,
    pub i_checksum_hi: u16,
    pub i_ctime_extra: u32,
    pub i_mtime_extra: u32,
    pub i_atime_extra: u32,
    pub i_crtime: u32,
    pub i_crtime_extra: u32,
    pub i_version_hi: u32,
}

impl Inode {
    pub const ROOT_INODE: u32 = 2;
    pub const JOURNAL_INODE: u32 = 8;
    pub const UNDEL_DIR_INODE: u32 = 6;
    pub const LOST_AND_FOUND_INODE: u32 = 11;
    pub const INODE_MODE_FILE: u16 = 0x8000;
    pub const INODE_MODE_DIR: u16 = 0x4000;
    pub const INODE_MODE_SYMLINK: u16 = 0xA000;
    pub const INODE_MODE_TYPE_MASK: u16 = 0xF000;
    pub const INODE_MODE_PERM_MASK: u16 = 0x0FFF;
    pub const INODE_BLOCK_SIZE: usize = 512;
    pub const GOOD_OLD_INODE_SIZE: u16 = 128;
    pub const INODE_FLAG_EXTENTS: u32 = 0x00080000;

    pub fn parse(bytes: &[u8]) -> Result<Self> {
        match Parse::parse(bytes).finish() {
            Ok((_, descriptor)) => Ok(descriptor),
            Err(e) => Err(Ext4Error::Parse(format!("{:?}", e))),
        }
    }

    pub fn is_directory(&self) -> bool {
        (self.mode & Self::INODE_MODE_TYPE_MASK) == Self::INODE_MODE_DIR
    }

    pub fn is_regular_file(&self) -> bool {
        (self.mode & Self::INODE_MODE_TYPE_MASK) == Self::INODE_MODE_FILE
    }

    pub fn is_symlink(&self) -> bool {
        (self.mode & Self::INODE_MODE_TYPE_MASK) == Self::INODE_MODE_SYMLINK
    }

    pub fn uses_extents(&self) -> bool {
        (self.flags & Self::INODE_FLAG_EXTENTS) != 0
    }
}
