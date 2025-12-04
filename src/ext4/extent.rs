use crate::{Ext4Error, Result};
use nom::Finish;
use nom_derive::{NomLE, Parse};

use super::Ext4Lblk;

#[derive(Debug, Default, Clone, Copy, NomLE)]
#[repr(C)]
pub struct ExtentHeader {
    #[nom(Verify = "*magic == 0xF30A")]
    pub magic: u16,
    pub entries_count: u16,
    pub max_entries_count: u16,
    pub depth: u16,
    pub generation: u32,
}

impl ExtentHeader {
    pub const SIZE: usize = 12;

    pub fn parse(bytes: &[u8]) -> Result<Self> {
        match Parse::parse(bytes).finish() {
            Ok((_, descriptor)) => Ok(descriptor),
            Err(e) => Err(Ext4Error::Parse(format!("{:?}", e))),
        }
    }
}

#[derive(Debug, Default, Clone, Copy, NomLE)]
#[repr(C)]
pub struct ExtentIndex {
    pub first_block: u32,
    pub leaf_lo: u32,
    pub leaf_hi: u16,
    pub padding: u16,
}

impl ExtentIndex {
    pub const SIZE: usize = 12;
    pub const MAX_INDEX_COUNT: usize = 340;

    pub fn parse(bytes: &[u8]) -> Result<Self> {
        match Parse::parse(bytes).finish() {
            Ok((_, descriptor)) => Ok(descriptor),
            Err(e) => Err(Ext4Error::Parse(format!("{:?}", e))),
        }
    }

    pub fn leaf_block(&self) -> u64 {
        ((self.leaf_hi as u64) << 32) | (self.leaf_lo as u64)
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, NomLE)]
#[repr(C)]
pub struct Extent {
    pub first_block: u32,
    pub block_count: u16,
    pub start_hi: u16,
    pub start_lo: u32,
}

impl Extent {
    pub const SIZE: usize = 12;
    pub const INIT_MAX_LEN: u16 = 32768;
    pub const UNWRITTEN_MAX_LEN: u16 = 65535;
    pub const EXT_MAX_BLOCKS: Ext4Lblk = u32::MAX;

    pub fn parse(bytes: &[u8]) -> Result<Self> {
        match Parse::parse(bytes).finish() {
            Ok((_, descriptor)) => Ok(descriptor),
            Err(e) => Err(Ext4Error::Parse(format!("{:?}", e))),
        }
    }

    pub fn start_block(&self) -> u64 {
        ((self.start_hi as u64) << 32) | (self.start_lo as u64)
    }

    pub fn is_unwritten(&self) -> bool {
        self.block_count > Self::INIT_MAX_LEN
    }

    pub fn get_actual_len(&self) -> u16 {
        if self.is_unwritten() {
            self.block_count - Self::INIT_MAX_LEN
        } else {
            self.block_count
        }
    }
}
