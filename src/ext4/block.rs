use bitflags::bitflags;
use nom::Finish;
use nom_derive::{NomLE, Parse};

use crate::{Ext4Error, Result};

#[derive(Debug, Clone, Copy, NomLE)]
#[repr(C, packed)]
pub struct BlockGroupDescriptor {
    block_bitmap_lo: u32,
    inode_bitmap_lo: u32,
    inode_table_first_block_lo: u32,
    free_blocks_count_lo: u16,
    free_inodes_count_lo: u16,
    used_dirs_count_lo: u16,
    #[nom(Parse = "Flags::parse")]
    flags: Flags,
    exclude_bitmap_lo: u32,
    block_bitmap_csum_lo: u16,
    inode_bitmap_csum_lo: u16,
    itable_unused_lo: u16,
    checksum: u16,

    #[nom(Cond = "i.len() >= 32")]
    block_bitmap_hi: Option<u32>,
    #[nom(Cond = "i.len() >= 32")]
    inode_bitmap_hi: Option<u32>,
    #[nom(Cond = "i.len() >= 32")]
    inode_table_first_block_hi: Option<u32>,
    #[nom(Cond = "i.len() >= 32")]
    free_blocks_count_hi: Option<u16>,
    #[nom(Cond = "i.len() >= 32")]
    free_inodes_count_hi: Option<u16>,
    #[nom(Cond = "i.len() >= 32")]
    used_dirs_count_hi: Option<u16>,
    #[nom(Cond = "i.len() >= 32")]
    itable_unused_hi: Option<u16>,
    #[nom(Cond = "i.len() >= 32")]
    exclude_bitmap_hi: Option<u32>,
    #[nom(Cond = "i.len() >= 32")]
    block_bitmap_csum_hi: Option<u16>,
    #[nom(Cond = "i.len() >= 32")]
    inode_bitmap_csum_hi: Option<u16>,
    #[nom(Cond = "i.len() >= 32")]
    reserved: Option<u32>,
}

impl BlockGroupDescriptor {
    pub const MIN_SIZE: u16 = 32;
    pub const MAX_SIZE: u16 = 64;

    pub fn parse(bytes: &[u8]) -> Result<Self> {
        match Parse::parse(bytes).finish() {
            Ok((_, descriptor)) => Ok(descriptor),
            Err(e) => Err(Ext4Error::Parse(format!("{:?}", e))),
        }
    }

    pub fn block_bitmap(&self) -> u64 {
        ((self.block_bitmap_hi.unwrap_or(0) as u64) << 32) | (self.block_bitmap_lo as u64)
    }

    pub fn inode_bitmap(&self) -> u64 {
        ((self.inode_bitmap_hi.unwrap_or(0) as u64) << 32) | (self.inode_bitmap_lo as u64)
    }

    pub fn inode_table_first_block(&self) -> u64 {
        ((self.inode_table_first_block_hi.unwrap_or(0) as u64) << 32)
            | (self.inode_table_first_block_lo as u64)
    }

    pub fn free_blocks_count(&self) -> u32 {
        ((self.free_blocks_count_hi.unwrap_or(0) as u32) << 16) | (self.free_blocks_count_lo as u32)
    }

    pub fn free_inodes_count(&self) -> u32 {
        ((self.free_inodes_count_hi.unwrap_or(0) as u32) << 16) | (self.free_inodes_count_lo as u32)
    }

    pub fn used_dirs_count(&self) -> u32 {
        ((self.used_dirs_count_hi.unwrap_or(0) as u32) << 16) | (self.used_dirs_count_lo as u32)
    }

    pub fn itable_unused(&self) -> u32 {
        ((self.itable_unused_hi.unwrap_or(0) as u32) << 16) | (self.itable_unused_lo as u32)
    }

    pub fn exclude_bitmap(&self) -> u64 {
        ((self.exclude_bitmap_hi.unwrap_or(0) as u64) << 32) | (self.exclude_bitmap_lo as u64)
    }
}

bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct Flags: u16 {
        const InodeTableUninitialized = 0x0001;
        const BlockBitmapUninitialized = 0x0002;
        const InodeTableZeroed = 0x0004;
    }
}

impl Flags {
    pub fn parse(input: &[u8]) -> nom::IResult<&[u8], Self> {
        let (input, bits) = nom::number::complete::le_u16(input)?;
        Ok((input, Flags::from_bits_truncate(bits)))
    }
}
