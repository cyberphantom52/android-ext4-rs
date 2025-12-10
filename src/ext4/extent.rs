use crate::{Error, ParseContext, Result};
use nom::Finish;
use nom_derive::{NomLE, Parse};

use super::Ext4Lblk;

#[derive(Debug, Default, Clone, Copy, NomLE)]
#[repr(C)]
pub struct ExtentHeader {
    #[nom(Verify = "*magic == 0xF30A")]
    magic: u16,
    entries_count: u16,
    max_entries_count: u16,
    depth: u16,
    generation: u32,
}

impl ExtentHeader {
    pub const SIZE: usize = 12;

    pub fn parse(bytes: &[u8]) -> Result<Self> {
        match Parse::parse(bytes).finish() {
            Ok((_, descriptor)) => Ok(descriptor),
            Err(e) => Err(Error::nom_parse(ParseContext::ExtentHeader, e)),
        }
    }

    pub fn entries_count(&self) -> u16 {
        self.entries_count
    }

    pub fn depth(&self) -> u16 {
        self.depth
    }
}

#[derive(Debug, Default, Clone, Copy, NomLE)]
#[repr(C)]
pub struct ExtentIndex {
    first_block: u32,
    leaf_lo: u32,
    leaf_hi: u16,
    padding: u16,
}

impl ExtentIndex {
    pub const SIZE: usize = 12;
    pub const MAX_INDEX_COUNT: usize = 340;

    pub fn parse(bytes: &[u8]) -> Result<Self> {
        match Parse::parse(bytes).finish() {
            Ok((_, descriptor)) => Ok(descriptor),
            Err(e) => Err(Error::nom_parse(ParseContext::ExtentIndex, e)),
        }
    }

    pub fn leaf_block(&self) -> u64 {
        ((self.leaf_hi as u64) << 32) | (self.leaf_lo as u64)
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, NomLE)]
#[repr(C)]
pub struct Extent {
    first_block: u32,
    block_count: u16,
    start_hi: u16,
    start_lo: u32,
}

impl Extent {
    pub const SIZE: usize = 12;
    pub const INIT_MAX_LEN: u16 = 32768;
    pub const UNWRITTEN_MAX_LEN: u16 = 65535;
    pub const EXT_MAX_BLOCKS: Ext4Lblk = u32::MAX;

    pub fn parse(bytes: &[u8]) -> Result<Self> {
        match Parse::parse(bytes).finish() {
            Ok((_, descriptor)) => Ok(descriptor),
            Err(e) => Err(Error::nom_parse(ParseContext::Extent, e)),
        }
    }

    pub fn first_block(&self) -> u64 {
        self.first_block as u64
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
