use nom::IResult;
use nom::number::complete::{le_u8, le_u16, le_u32, le_u64};
use std::cmp::Ordering;

use crate::ext4::{block::BlockGroupDescriptor, inode::Inode};

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
    pub uuid: [u8; 16],
    pub volume_name: [u8; 16],
    pub last_mounted: [u8; 64],
    pub algorithm_usage_bitmap: u32,
    pub s_prealloc_blocks: u8,
    pub s_prealloc_dir_blocks: u8,
    pub s_reserved_gdt_blocks: u16,
    pub journal_uuid: [u8; 16],
    pub journal_inode_number: u32,
    pub journal_dev: u32,
    pub last_orphan: u32,
    pub hash_seed: [u32; 4],
    pub default_hash_version: u8,
    pub journal_backup_type: u8,
    pub desc_size: u16,
    pub default_mount_opts: u32,
    pub first_meta_bg: u32,
    pub mkfs_time: u32,
    pub journal_blocks: [u32; 17],
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
    pub first_error_func: [u8; 32],
    pub first_error_line: u32,
    pub last_error_time: u32,
    pub last_error_ino: u32,
    pub last_error_line: u32,
    pub last_error_block: u64,
    pub last_error_func: [u8; 32],
    pub mount_opts: [u8; 64],
    pub usr_quota_inum: u32,
    pub grp_quota_inum: u32,
    pub overhead_clusters: u32,
    pub backup_bgs: [u32; 2],
    pub encrypt_algos: [u8; 4],
    pub encrypt_pw_salt: [u8; 16],
    pub lpf_ino: u32,
    pub padding: [u32; 100],
    pub checksum: u32,
}

impl Superblock {
    pub const SUPERBLOCK_OFFSET: u64 = 1024;
    pub const EXT4_SUPERBLOCK_MAGIC: u16 = 0xEF53;
    pub const EXT4_SUPERBLOCK_OS_HURD: u32 = 1;

    pub fn parse(input: &[u8]) -> IResult<&[u8], Self> {
        let (input, inodes_count) = le_u32(input)?;
        let (input, blocks_count_lo) = le_u32(input)?;
        let (input, reserved_blocks_count_lo) = le_u32(input)?;
        let (input, free_blocks_count_lo) = le_u32(input)?;
        let (input, free_inodes_count) = le_u32(input)?;
        let (input, first_data_block) = le_u32(input)?;
        let (input, log_block_size) = le_u32(input)?;
        let (input, log_cluster_size) = le_u32(input)?;
        let (input, blocks_per_group) = le_u32(input)?;
        let (input, frags_per_group) = le_u32(input)?;
        let (input, inodes_per_group) = le_u32(input)?;
        let (input, mount_time) = le_u32(input)?;
        let (input, write_time) = le_u32(input)?;
        let (input, mount_count) = le_u16(input)?;
        let (input, max_mount_count) = le_u16(input)?;
        let (input, magic) = le_u16(input)?;
        let (input, state) = le_u16(input)?;
        let (input, errors) = le_u16(input)?;
        let (input, minor_rev_level) = le_u16(input)?;
        let (input, last_check_time) = le_u32(input)?;
        let (input, check_interval) = le_u32(input)?;
        let (input, creator_os) = le_u32(input)?;
        let (input, rev_level) = le_u32(input)?;
        let (input, def_resuid) = le_u16(input)?;
        let (input, def_resgid) = le_u16(input)?;
        let (input, first_inode) = le_u32(input)?;
        let (input, inode_size) = le_u16(input)?;
        let (input, block_group_index) = le_u16(input)?;
        let (input, features_compatible) = le_u32(input)?;
        let (input, features_incompatible) = le_u32(input)?;
        let (input, features_read_only) = le_u32(input)?;
        let (input, uuid) = nom::bytes::complete::take(16usize)(input)?;
        let (input, volume_name) = nom::bytes::complete::take(16usize)(input)?;
        let (input, last_mounted) = nom::bytes::complete::take(64usize)(input)?;
        let (input, algorithm_usage_bitmap) = le_u32(input)?;
        let (input, s_prealloc_blocks) = le_u8(input)?;
        let (input, s_prealloc_dir_blocks) = le_u8(input)?;
        let (input, s_reserved_gdt_blocks) = le_u16(input)?;
        let (input, journal_uuid) = nom::bytes::complete::take(16usize)(input)?;
        let (input, journal_inode_number) = le_u32(input)?;
        let (input, journal_dev) = le_u32(input)?;
        let (input, last_orphan) = le_u32(input)?;
        let (input, hash_seed_0) = le_u32(input)?;
        let (input, hash_seed_1) = le_u32(input)?;
        let (input, hash_seed_2) = le_u32(input)?;
        let (input, hash_seed_3) = le_u32(input)?;
        let (input, default_hash_version) = le_u8(input)?;
        let (input, journal_backup_type) = le_u8(input)?;
        let (input, desc_size) = le_u16(input)?;
        let (input, default_mount_opts) = le_u32(input)?;
        let (input, first_meta_bg) = le_u32(input)?;
        let (input, mkfs_time) = le_u32(input)?;
        let (input, journal_blocks) = nom::multi::count(le_u32, 17)(input)?;
        let (input, blocks_count_hi) = le_u32(input)?;
        let (input, reserved_blocks_count_hi) = le_u32(input)?;
        let (input, free_blocks_count_hi) = le_u32(input)?;
        let (input, min_extra_isize) = le_u16(input)?;
        let (input, want_extra_isize) = le_u16(input)?;
        let (input, flags) = le_u32(input)?;
        let (input, raid_stride) = le_u16(input)?;
        let (input, mmp_interval) = le_u16(input)?;
        let (input, mmp_block) = le_u64(input)?;
        let (input, raid_stripe_width) = le_u32(input)?;
        let (input, log_groups_per_flex) = le_u8(input)?;
        let (input, checksum_type) = le_u8(input)?;
        let (input, reserved_pad) = le_u16(input)?;
        let (input, kbytes_written) = le_u64(input)?;
        let (input, snapshot_inum) = le_u32(input)?;
        let (input, snapshot_id) = le_u32(input)?;
        let (input, snapshot_r_blocks_count) = le_u64(input)?;
        let (input, snapshot_list) = le_u32(input)?;
        let (input, error_count) = le_u32(input)?;
        let (input, first_error_time) = le_u32(input)?;
        let (input, first_error_ino) = le_u32(input)?;
        let (input, first_error_block) = le_u64(input)?;
        let (input, first_error_func) = nom::bytes::complete::take(32usize)(input)?;
        let (input, first_error_line) = le_u32(input)?;
        let (input, last_error_time) = le_u32(input)?;
        let (input, last_error_ino) = le_u32(input)?;
        let (input, last_error_line) = le_u32(input)?;
        let (input, last_error_block) = le_u64(input)?;
        let (input, last_error_func) = nom::bytes::complete::take(32usize)(input)?;
        let (input, mount_opts) = nom::bytes::complete::take(64usize)(input)?;
        let (input, usr_quota_inum) = le_u32(input)?;
        let (input, grp_quota_inum) = le_u32(input)?;
        let (input, overhead_clusters) = le_u32(input)?;
        let (input, backup_bgs_0) = le_u32(input)?;
        let (input, backup_bgs_1) = le_u32(input)?;
        let (input, encrypt_algos) = nom::bytes::complete::take(4usize)(input)?;
        let (input, encrypt_pw_salt) = nom::bytes::complete::take(16usize)(input)?;
        let (input, lpf_ino) = le_u32(input)?;
        let (input, padding) = nom::multi::count(le_u32, 100)(input)?;
        let (input, checksum) = le_u32(input)?;

        let mut uuid_arr = [0u8; 16];
        uuid_arr.copy_from_slice(uuid);
        let mut volume_name_arr = [0u8; 16];
        volume_name_arr.copy_from_slice(volume_name);
        let mut last_mounted_arr = [0u8; 64];
        last_mounted_arr.copy_from_slice(last_mounted);
        let mut journal_uuid_arr = [0u8; 16];
        journal_uuid_arr.copy_from_slice(journal_uuid);
        let mut first_error_func_arr = [0u8; 32];
        first_error_func_arr.copy_from_slice(first_error_func);
        let mut last_error_func_arr = [0u8; 32];
        last_error_func_arr.copy_from_slice(last_error_func);
        let mut mount_opts_arr = [0u8; 64];
        mount_opts_arr.copy_from_slice(mount_opts);
        let mut encrypt_algos_arr = [0u8; 4];
        encrypt_algos_arr.copy_from_slice(encrypt_algos);
        let mut encrypt_pw_salt_arr = [0u8; 16];
        encrypt_pw_salt_arr.copy_from_slice(encrypt_pw_salt);

        let mut journal_blocks_arr = [0u32; 17];
        journal_blocks_arr.copy_from_slice(&journal_blocks);
        let mut padding_arr = [0u32; 100];
        padding_arr.copy_from_slice(&padding);

        Ok((
            input,
            Superblock {
                inodes_count,
                blocks_count_lo,
                reserved_blocks_count_lo,
                free_blocks_count_lo,
                free_inodes_count,
                first_data_block,
                log_block_size,
                log_cluster_size,
                blocks_per_group,
                frags_per_group,
                inodes_per_group,
                mount_time,
                write_time,
                mount_count,
                max_mount_count,
                magic,
                state,
                errors,
                minor_rev_level,
                last_check_time,
                check_interval,
                creator_os,
                rev_level,
                def_resuid,
                def_resgid,
                first_inode,
                inode_size,
                block_group_index,
                features_compatible,
                features_incompatible,
                features_read_only,
                uuid: uuid_arr,
                volume_name: volume_name_arr,
                last_mounted: last_mounted_arr,
                algorithm_usage_bitmap,
                s_prealloc_blocks,
                s_prealloc_dir_blocks,
                s_reserved_gdt_blocks,
                journal_uuid: journal_uuid_arr,
                journal_inode_number,
                journal_dev,
                last_orphan,
                hash_seed: [hash_seed_0, hash_seed_1, hash_seed_2, hash_seed_3],
                default_hash_version,
                journal_backup_type,
                desc_size,
                default_mount_opts,
                first_meta_bg,
                mkfs_time,
                journal_blocks: journal_blocks_arr,
                blocks_count_hi,
                reserved_blocks_count_hi,
                free_blocks_count_hi,
                min_extra_isize,
                want_extra_isize,
                flags,
                raid_stride,
                mmp_interval,
                mmp_block,
                raid_stripe_width,
                log_groups_per_flex,
                checksum_type,
                reserved_pad,
                kbytes_written,
                snapshot_inum,
                snapshot_id,
                snapshot_r_blocks_count,
                snapshot_list,
                error_count,
                first_error_time,
                first_error_ino,
                first_error_block,
                first_error_func: first_error_func_arr,
                first_error_line,
                last_error_time,
                last_error_ino,
                last_error_line,
                last_error_block,
                last_error_func: last_error_func_arr,
                mount_opts: mount_opts_arr,
                usr_quota_inum,
                grp_quota_inum,
                overhead_clusters,
                backup_bgs: [backup_bgs_0, backup_bgs_1],
                encrypt_algos: encrypt_algos_arr,
                encrypt_pw_salt: encrypt_pw_salt_arr,
                lpf_ino,
                padding: padding_arr,
                checksum,
            },
        ))
    }

    pub fn inode_size_file(&self, inode: &Inode) -> u64 {
        let mode = inode.mode;
        let mut v = inode.size as u64;
        if self.rev_level > 0
            && (mode & Inode::EXT4_INODE_MODE_TYPE_MASK) == Inode::EXT4_INODE_MODE_FILE
        {
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
