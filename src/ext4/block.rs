use nom::IResult;
use nom::number::complete::{le_u16, le_u32};

#[derive(Debug, Default, Clone, Copy)]
#[repr(C, packed)]
pub struct BlockGroupDescriptor {
    block_bitmap_lo: u32,
    inode_bitmap_lo: u32,
    inode_table_first_block_lo: u32,
    free_blocks_count_lo: u16,
    free_inodes_count_lo: u16,
    used_dirs_count_lo: u16,
    flags: u16,
    exclude_bitmap_lo: u32,
    block_bitmap_csum_lo: u16,
    inode_bitmap_csum_lo: u16,
    itable_unused_lo: u16,
    checksum: u16,
    block_bitmap_hi: u32,
    inode_bitmap_hi: u32,
    inode_table_first_block_hi: u32,
    free_blocks_count_hi: u16,
    free_inodes_count_hi: u16,
    used_dirs_count_hi: u16,
    itable_unused_hi: u16,
    exclude_bitmap_hi: u32,
    block_bitmap_csum_hi: u16,
    inode_bitmap_csum_hi: u16,
    reserved: u32,
}

impl BlockGroupDescriptor {
    pub const MIN_SIZE: u16 = 32;
    pub const MAX_SIZE: u16 = 64;

    pub fn parse(input: &[u8]) -> IResult<&[u8], Self> {
        let original_input = input;
        let (input, block_bitmap_lo) = le_u32(input)?;
        let (input, inode_bitmap_lo) = le_u32(input)?;
        let (input, inode_table_first_block_lo) = le_u32(input)?;
        let (input, free_blocks_count_lo) = le_u16(input)?;
        let (input, free_inodes_count_lo) = le_u16(input)?;
        let (input, used_dirs_count_lo) = le_u16(input)?;
        let (input, flags) = le_u16(input)?;
        let (input, exclude_bitmap_lo) = le_u32(input)?;
        let (input, block_bitmap_csum_lo) = le_u16(input)?;
        let (input, inode_bitmap_csum_lo) = le_u16(input)?;
        let (input, itable_unused_lo) = le_u16(input)?;
        let (input, checksum) = le_u16(input)?;

        let bytes_read = original_input.len() - input.len();
        let remaining = original_input.len() - bytes_read;

        let (
            block_bitmap_hi,
            inode_bitmap_hi,
            inode_table_first_block_hi,
            free_blocks_count_hi,
            free_inodes_count_hi,
            used_dirs_count_hi,
            itable_unused_hi,
            exclude_bitmap_hi,
            block_bitmap_csum_hi,
            inode_bitmap_csum_hi,
            reserved,
        ) = if remaining >= 32 {
            let (input, block_bitmap_hi) = le_u32(input)?;
            let (input, inode_bitmap_hi) = le_u32(input)?;
            let (input, inode_table_first_block_hi) = le_u32(input)?;
            let (input, free_blocks_count_hi) = le_u16(input)?;
            let (input, free_inodes_count_hi) = le_u16(input)?;
            let (input, used_dirs_count_hi) = le_u16(input)?;
            let (input, itable_unused_hi) = le_u16(input)?;
            let (input, exclude_bitmap_hi) = le_u32(input)?;
            let (input, block_bitmap_csum_hi) = le_u16(input)?;
            let (input, inode_bitmap_csum_hi) = le_u16(input)?;
            let (_input, reserved) = le_u32(input)?;
            (
                block_bitmap_hi,
                inode_bitmap_hi,
                inode_table_first_block_hi,
                free_blocks_count_hi,
                free_inodes_count_hi,
                used_dirs_count_hi,
                itable_unused_hi,
                exclude_bitmap_hi,
                block_bitmap_csum_hi,
                inode_bitmap_csum_hi,
                reserved,
            )
        } else {
            (0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0)
        };

        Ok((
            input,
            BlockGroupDescriptor {
                block_bitmap_lo,
                inode_bitmap_lo,
                inode_table_first_block_lo,
                free_blocks_count_lo,
                free_inodes_count_lo,
                used_dirs_count_lo,
                flags,
                exclude_bitmap_lo,
                block_bitmap_csum_lo,
                inode_bitmap_csum_lo,
                itable_unused_lo,
                checksum,
                block_bitmap_hi,
                inode_bitmap_hi,
                inode_table_first_block_hi,
                free_blocks_count_hi,
                free_inodes_count_hi,
                used_dirs_count_hi,
                itable_unused_hi,
                exclude_bitmap_hi,
                block_bitmap_csum_hi,
                inode_bitmap_csum_hi,
                reserved,
            },
        ))
    }

    pub fn block_bitmap(&self) -> u64 {
        ((self.block_bitmap_hi as u64) << 32) | (self.block_bitmap_lo as u64)
    }

    pub fn inode_bitmap(&self) -> u64 {
        ((self.inode_bitmap_hi as u64) << 32) | (self.inode_bitmap_lo as u64)
    }

    pub fn inode_table_first_block(&self) -> u64 {
        ((self.inode_table_first_block_hi as u64) << 32) | (self.inode_table_first_block_lo as u64)
    }

    pub fn free_blocks_count(&self) -> u32 {
        ((self.free_blocks_count_hi as u32) << 16) | (self.free_blocks_count_lo as u32)
    }

    pub fn free_inodes_count(&self) -> u32 {
        ((self.free_inodes_count_hi as u32) << 16) | (self.free_inodes_count_lo as u32)
    }

    pub fn used_dirs_count(&self) -> u32 {
        ((self.used_dirs_count_hi as u32) << 16) | (self.used_dirs_count_lo as u32)
    }

    pub fn itable_unused(&self) -> u32 {
        ((self.itable_unused_hi as u32) << 16) | (self.itable_unused_lo as u32)
    }

    pub fn exclude_bitmap(&self) -> u64 {
        ((self.exclude_bitmap_hi as u64) << 32) | (self.exclude_bitmap_lo as u64)
    }
}
