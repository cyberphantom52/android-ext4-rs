use crate::ext4::{block::BlockGroupDescriptor, inode::Inode};
use crate::{Ext4Error, Result};
use nom::Finish;
use nom_derive::{NomLE, Parse};
use std::cmp::Ordering;

#[repr(C)]
#[derive(Debug, Clone, PartialEq, Eq, NomLE)]
pub struct Superblock {
    pub inodes_count: u32,
    pub blocks_count_lo: u32,
    pub reserved_blocks_count_lo: u32,
    pub free_blocks_count_lo: u32,
    pub free_inodes_count: u32,
    pub first_data_block: u32,
    pub log_block_size: u32,
    pub log_cluster_size: u32,
    pub blocks_per_group: u32,
    pub frags_per_group: u32,
    pub inodes_per_group: u32,
    pub mount_time: u32,
    pub write_time: u32,
    pub mount_count: u16,
    pub max_mount_count: u16,

    #[nom(Verify = "*magic == 0xEF53")]
    pub magic: u16,

    pub state: u16,
    pub errors: u16,
    pub minor_rev_level: u16,
    pub last_check_time: u32,
    pub check_interval: u32,
    pub creator_os: u32,
    pub rev_level: u32,
    pub def_resuid: u16,
    pub def_resgid: u16,
    pub first_inode: u32,
    pub inode_size: u16,
    pub block_group_index: u16,
    pub features_compatible: u32,
    pub features_incompatible: u32,
    pub features_read_only: u32,

    #[nom(Count = "16")]
    pub uuid: Vec<u8>,

    #[nom(Count = "16")]
    pub volume_name: Vec<u8>,

    #[nom(Count = "64")]
    pub last_mounted: Vec<u8>,

    pub algorithm_usage_bitmap: u32,
    pub s_prealloc_blocks: u8,
    pub s_prealloc_dir_blocks: u8,
    pub s_reserved_gdt_blocks: u16,

    #[nom(Count = "16")]
    pub journal_uuid: Vec<u8>,

    pub journal_inode_number: u32,
    pub journal_dev: u32,
    pub last_orphan: u32,

    #[nom(Count = "4")]
    pub hash_seed: Vec<u32>,

    pub default_hash_version: u8,
    pub journal_backup_type: u8,
    pub desc_size: u16,
    pub default_mount_opts: u32,
    pub first_meta_bg: u32,
    pub mkfs_time: u32,

    #[nom(Count = "17")]
    pub journal_blocks: Vec<u32>,

    pub blocks_count_hi: u32,
    pub reserved_blocks_count_hi: u32,
    pub free_blocks_count_hi: u32,
    pub min_extra_isize: u16,
    pub want_extra_isize: u16,
    pub flags: u32,
    pub raid_stride: u16,
    pub mmp_interval: u16,
    pub mmp_block: u64,
    pub raid_stripe_width: u32,
    pub log_groups_per_flex: u8,
    pub checksum_type: u8,
    pub reserved_pad: u16,
    pub kbytes_written: u64,
    pub snapshot_inum: u32,
    pub snapshot_id: u32,
    pub snapshot_r_blocks_count: u64,
    pub snapshot_list: u32,
    pub error_count: u32,
    pub first_error_time: u32,
    pub first_error_ino: u32,
    pub first_error_block: u64,

    #[nom(Count = "32")]
    pub first_error_func: Vec<u8>,

    pub first_error_line: u32,
    pub last_error_time: u32,
    pub last_error_ino: u32,
    pub last_error_line: u32,
    pub last_error_block: u64,

    #[nom(Count = "32")]
    pub last_error_func: Vec<u8>,

    #[nom(Count = "64")]
    pub mount_opts: Vec<u8>,

    pub usr_quota_inum: u32,
    pub grp_quota_inum: u32,
    pub overhead_clusters: u32,

    #[nom(Count = "2")]
    pub backup_bgs: Vec<u32>,

    #[nom(Count = "4")]
    pub encrypt_algos: Vec<u8>,

    #[nom(Count = "16")]
    pub encrypt_pw_salt: Vec<u8>,

    pub lpf_ino: u32,

    #[nom(Count = "100")]
    pub padding: Vec<u32>,

    pub checksum: u32,
}

impl Superblock {
    pub const SUPERBLOCK_OFFSET: u64 = 1024;
    pub const EXT4_SUPERBLOCK_OS_HURD: u32 = 1;

    pub fn parse(bytes: &[u8]) -> Result<Self> {
        match Parse::parse(bytes).finish() {
            Ok((_, superblock)) => Ok(superblock),
            Err(e) => Err(Ext4Error::Parse(format!("{:?}", e))),
        }
    }

    pub fn inode_size_file(&self, inode: &Inode) -> u64 {
        let mode = inode.mode;
        let mut v = inode.size as u64;
        if self.rev_level > 0 && (mode & Inode::INODE_MODE_TYPE_MASK) == Inode::INODE_MODE_FILE {
            let hi = (inode.size_hi as u64) << 32;
            v |= hi;
        }
        v
    }

    pub fn block_size(&self) -> u32 {
        1024 << self.log_block_size
    }

    pub fn block_group_count(&self) -> u32 {
        let blocks_count = (self.blocks_count_hi as u64) << 32 | self.blocks_count_lo as u64;
        let blocks_per_group = self.blocks_per_group as u64;
        let mut block_group_count = blocks_count / blocks_per_group;
        if !blocks_count.is_multiple_of(blocks_per_group) {
            block_group_count += 1;
        }
        block_group_count as u32
    }

    pub fn blocks_count(&self) -> u64 {
        ((self.blocks_count_hi as u64) << 32) | self.blocks_count_lo as u64
    }

    pub fn descriptor_size(&self) -> u16 {
        let size = self.desc_size;
        match size.cmp(&BlockGroupDescriptor::MIN_SIZE) {
            Ordering::Less => BlockGroupDescriptor::MIN_SIZE,
            _ => size,
        }
    }

    pub fn inodes_in_group_cnt(&self, bgid: u32) -> u32 {
        let block_group_count = self.block_group_count();
        let inodes_per_group = self.inodes_per_group;
        let total_inodes = self.inodes_count;
        if bgid < block_group_count - 1 {
            inodes_per_group
        } else {
            total_inodes - ((block_group_count - 1) * inodes_per_group)
        }
    }

    pub fn free_blocks_count(&self) -> u64 {
        self.free_blocks_count_lo as u64 | ((self.free_blocks_count_hi as u64) << 32)
    }
}
