use super::Ext4Lblk;

#[derive(Debug, Default, Clone, Copy)]
#[repr(C)]
pub struct ExtentHeader {
    /// Magic number, 0xF30A.
    pub magic: u16,

    /// Number of valid entries following the header.
    pub entries_count: u16,

    /// Maximum number of entries that could follow the header.
    pub max_entries_count: u16,

    /// Depth of this extent node in the extent tree. Depth 0 indicates that this node points to data blocks.
    pub depth: u16,

    /// Generation of the tree (used by Lustre, but not standard in ext4).
    pub generation: u32,
}

/// Structure representing an index node within an extent tree.
#[derive(Debug, Default, Clone, Copy)]
#[repr(C)]
pub struct ExtentIndex {
    /// Block number from which this index node starts.
    pub first_block: u32,

    /// Lower 32-bits of the block number to which this index points.
    pub leaf_lo: u32,

    /// Upper 16-bits of the block number to which this index points.
    pub leaf_hi: u16,

    /// Padding for alignment.
    pub padding: u16,
}

/// Structure representing an Ext4 extent.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
pub struct Extent {
    /// First file block number that this extent covers.
    pub first_block: u32,

    /// Number of blocks covered by this extent.
    pub block_count: u16,

    /// Upper 16-bits of the block number to which this extent points.
    pub start_hi: u16,

    /// Lower 32-bits of the block number to which this extent points.
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
}
