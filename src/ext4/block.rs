use std::cmp::Ordering;

use crate::ext4::superblock::Superblock;

/// Represents the structure of an Ext4 block group descriptor.
#[derive(Debug, Default, Clone, Copy)]
#[repr(C, packed)]
pub struct BlockGroupDescriptor {
    /// Block bitmap block
    block_bitmap_lo: u32,
    /// Inode bitmap block
    inode_bitmap_lo: u32,
    /// Inode table block
    inode_table_first_block_lo: u32,
    /// Free blocks count
    free_blocks_count_lo: u16,
    /// Free inodes count
    free_inodes_count_lo: u16,
    /// Directories count
    used_dirs_count_lo: u16,
    /// EXT4_BG_flags (INODE_UNINIT, etc)
    flags: u16,
    /// Snapshot exclusion bitmap
    exclude_bitmap_lo: u32,
    /// crc32c(s_uuid+grp_num+bbitmap) LE
    block_bitmap_csum_lo: u16,
    /// crc32c(s_uuid+grp_num+ibitmap) LE
    inode_bitmap_csum_lo: u16,
    /// Unused inodes count
    itable_unused_lo: u16,
    /// crc16(sb_uuid+group+desc)
    checksum: u16,
    /// Block bitmap block MSB
    block_bitmap_hi: u32,
    /// Inode bitmap block MSB
    inode_bitmap_hi: u32,
    /// Inode table block MSB
    inode_table_first_block_hi: u32,
    /// Free blocks count MSB
    free_blocks_count_hi: u16,
    /// Free inodes count MSB
    free_inodes_count_hi: u16,
    /// Directories count MSB
    used_dirs_count_hi: u16,
    /// Unused inodes count MSB
    itable_unused_hi: u16,
    /// Snapshot exclusion bitmap MSB
    exclude_bitmap_hi: u32,
    /// crc32c(s_uuid+grp_num+bbitmap) BE
    block_bitmap_csum_hi: u16,
    /// crc32c(s_uuid+grp_num+ibitmap) BE
    inode_bitmap_csum_hi: u16,
    /// Padding
    reserved: u32,
}

impl BlockGroupDescriptor {
    pub const MIN_SIZE: u16 = 32;
    pub const MAX_SIZE: u16 = 64;
}
