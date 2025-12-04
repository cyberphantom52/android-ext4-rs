use nom::IResult;
use nom::number::complete::{le_u16, le_u32};

use super::Ext4Lblk;

#[derive(Debug, Default, Clone, Copy)]
#[repr(C)]
pub struct ExtentHeader {
    pub magic: u16,
    pub entries_count: u16,
    pub max_entries_count: u16,
    pub depth: u16,
    pub generation: u32,
}

impl ExtentHeader {
    pub fn parse(input: &[u8]) -> IResult<&[u8], Self> {
        let (input, magic) = le_u16(input)?;
        let (input, entries_count) = le_u16(input)?;
        let (input, max_entries_count) = le_u16(input)?;
        let (input, depth) = le_u16(input)?;
        let (input, generation) = le_u32(input)?;

        Ok((
            input,
            ExtentHeader {
                magic,
                entries_count,
                max_entries_count,
                depth,
                generation,
            },
        ))
    }
}

#[derive(Debug, Default, Clone, Copy)]
#[repr(C)]
pub struct ExtentIndex {
    pub first_block: u32,
    pub leaf_lo: u32,
    pub leaf_hi: u16,
    pub padding: u16,
}

impl ExtentIndex {
    pub fn parse(input: &[u8]) -> IResult<&[u8], Self> {
        let (input, first_block) = le_u32(input)?;
        let (input, leaf_lo) = le_u32(input)?;
        let (input, leaf_hi) = le_u16(input)?;
        let (input, padding) = le_u16(input)?;

        Ok((
            input,
            ExtentIndex {
                first_block,
                leaf_lo,
                leaf_hi,
                padding,
            },
        ))
    }

    pub fn leaf_block(&self) -> u64 {
        ((self.leaf_hi as u64) << 32) | (self.leaf_lo as u64)
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
pub struct Extent {
    pub first_block: u32,
    pub block_count: u16,
    pub start_hi: u16,
    pub start_lo: u32,
}

impl Extent {
    pub const EXT_INIT_MAX_LEN: u16 = 32768;
    pub const EXT_UNWRITTEN_MAX_LEN: u16 = 65535;
    pub const EXT_MAX_BLOCKS: Ext4Lblk = u32::MAX;
    pub const EXT4_EXTENT_MAGIC: u16 = 0xF30A;
    pub const EXT4_EXTENT_HEADER_SIZE: usize = 12;
    pub const EXT4_EXTENT_SIZE: usize = 12;
    pub const EXT4_EXTENT_INDEX_SIZE: usize = 12;
    pub const MAX_EXTENT_INDEX_COUNT: usize = 340;

    pub fn parse(input: &[u8]) -> IResult<&[u8], Self> {
        let (input, first_block) = le_u32(input)?;
        let (input, block_count) = le_u16(input)?;
        let (input, start_hi) = le_u16(input)?;
        let (input, start_lo) = le_u32(input)?;

        Ok((
            input,
            Extent {
                first_block,
                block_count,
                start_hi,
                start_lo,
            },
        ))
    }

    pub fn start_block(&self) -> u64 {
        ((self.start_hi as u64) << 32) | (self.start_lo as u64)
    }

    pub fn is_unwritten(&self) -> bool {
        self.block_count > Self::EXT_INIT_MAX_LEN
    }

    pub fn get_actual_len(&self) -> u16 {
        if self.is_unwritten() {
            self.block_count - Self::EXT_INIT_MAX_LEN
        } else {
            self.block_count
        }
    }
}
