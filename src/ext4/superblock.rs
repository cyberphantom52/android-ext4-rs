use crate::ext4::{block::BlockGroupDescriptor, inode::Inode};
use crate::{Ext4Error, Result};
use bitflags::bitflags;
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

    pub state: State,
    pub errors: ErrorPolicy,
    pub minor_rev_level: u16,
    pub last_check_time: u32,
    pub check_interval: u32,
    pub creator_os: CreatorOS,
    pub rev_level: Revision,
    pub def_resuid: u16,
    pub def_resgid: u16,
    pub first_inode: u32,
    pub inode_size: u16,
    pub block_group_index: u16,

    #[nom(Parse = "CompatibleFeatures::parse")]
    pub features_compatible: CompatibleFeatures,

    #[nom(Parse = "IncompatibleFeatures::parse")]
    pub features_incompatible: IncompatibleFeatures,

    #[nom(Parse = "ReadOnlyCompatibleFeatures::parse")]
    pub features_read_only: ReadOnlyCompatibleFeatures,

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

    pub default_hash_version: DefaultHashVersion,
    pub journal_backup_type: u8,
    pub desc_size: u16,

    #[nom(Parse = "DefaultMountOptions::parse")]
    pub default_mount_opts: DefaultMountOptions,

    pub first_meta_bg: u32,
    pub mkfs_time: u32,

    #[nom(Count = "17")]
    pub journal_blocks: Vec<u32>,

    pub blocks_count_hi: u32,
    pub reserved_blocks_count_hi: u32,
    pub free_blocks_count_hi: u32,
    pub min_extra_isize: u16,
    pub want_extra_isize: u16,
    #[nom(Parse = "Flags::parse")]
    pub flags: Flags,
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
    pub encrypt_algos: Vec<EncryptionAlgorithm>,

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
        let mut v = inode.size as u64;
        if self.rev_level == Revision::Dynamic && inode.is_regular_file() {
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, NomLE)]
#[repr(u16)]
pub enum State {
    Clean = 0x0001,
    Errors = 0x0002,
    Orphan = 0x0004,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, NomLE)]
#[repr(u16)]
pub enum ErrorPolicy {
    Continue = 1,
    ReadOnly = 2,
    Panic = 3,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, NomLE)]
#[repr(u32)]
pub enum Revision {
    Original = 0,
    Dynamic = 1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, NomLE)]
#[repr(u32)]
pub enum CreatorOS {
    Linux = 0,
    Hurd = 1,
    Masix = 2,
    FreeBSD = 3,
    Lites = 4,
}

bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct CompatibleFeatures: u32 {
        const DirectoryPreallocation = 0x0001;
        const IMagicInode = 0x0002;
        const HasJournal = 0x0004;
        const ExtendedAttributes = 0x0008;
        const ResizeInode = 0x0010;
        const DirectoryIndices = 0x0020;
        const LazyBlockGroups = 0x0040;
        const ExcludeInode = 0x0080;
        const ExcludeBitmap = 0x0100;
        const SparseSuper2 = 0x0200;
        const FastCommit = 0x0400;
        const OrphanFile = 0x1000;
    }
}

impl CompatibleFeatures {
    pub fn parse(input: &[u8]) -> nom::IResult<&[u8], Self> {
        let (input, bits) = nom::number::complete::le_u32(input)?;
        Ok((input, Self::from_bits_truncate(bits)))
    }
}

bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct IncompatibleFeatures: u32 {
        const Compression = 0x0001;
        const FileType = 0x0002;
        const NeedsRecovery = 0x0004;
        const JournalDevice = 0x0008;
        const MetaBlockGroups = 0x0010;
        const Extents = 0x0040;
        const Bit64 = 0x0080;
        const MultipleMountProtection = 0x0100;
        const FlexibleBlockGroups = 0x0200;
        const ExtendedAttributeInodes = 0x0400;
        const DirectoryData = 0x1000;
        const ChecksumSeed = 0x2000;
        const LargeDirectory = 0x4000;
        const InlineData = 0x8000;
        const EncryptedInodes = 0x10000;
    }
}

impl IncompatibleFeatures {
    pub fn parse(input: &[u8]) -> nom::IResult<&[u8], Self> {
        let (input, bits) = nom::number::complete::le_u32(input)?;
        Ok((input, Self::from_bits_truncate(bits)))
    }
}

bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct ReadOnlyCompatibleFeatures: u32 {
        const SparseSuper = 0x0001;
        const LargeFile = 0x0002;
        const BTreeDirectory = 0x0004;
        const HugeFile = 0x0008;
        const GroupDescriptorChecksum = 0x0010;
        const NoDirectoryLinkLimit = 0x0020;
        const ExtraInodeSize = 0x0040;
        const HasSnapshot = 0x0080;
        const Quota = 0x0100;
        const BigAlloc = 0x0200;
        const MetadataChecksum = 0x0400;
        const Replica = 0x0800;
        const ReadOnly = 0x1000;
        const ProjectQuota = 0x2000;
        const Verity = 0x8000;
        const OrphanPresent = 0x10000;
    }
}

impl ReadOnlyCompatibleFeatures {
    pub fn parse(input: &[u8]) -> nom::IResult<&[u8], Self> {
        let (input, bits) = nom::number::complete::le_u32(input)?;
        Ok((input, Self::from_bits_truncate(bits)))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, NomLE)]
#[repr(u8)]
pub enum DefaultHashVersion {
    Legacy = 0,
    HalfMD4 = 1,
    Tea = 2,
    LegacyUnsigned = 3,
    HalfMD4Unsigned = 4,
    TeaUnsigned = 5,
}

bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct DefaultMountOptions: u32 {
        const Debug = 0x0001;
        const BsdGroups = 0x0002;
        const ExtendedAttributeUser = 0x0004;
        const Acl = 0x0008;
        const Uid16 = 0x0010;
        const JournalModeData = 0x0020;
        const JournalModeOrdered = 0x0040;
        const JournalModeWriteback = 0x0060;
        const NoBarrier = 0x0100;
        const BlockValidity = 0x0200;
        const Discard = 0x0400;
        const NoDelayedAllocation = 0x0800;
    }
}

impl DefaultMountOptions {
    pub fn parse(input: &[u8]) -> nom::IResult<&[u8], Self> {
        let (input, bits) = nom::number::complete::le_u32(input)?;
        Ok((input, Self::from_bits_truncate(bits)))
    }
}

bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct Flags: u32 {
        const SignedDirectoryHash = 0x0001;
        const UnsignedDirectoryHash = 0x0002;
        const DevelopmentMode = 0x0004;
    }
}

impl Flags {
    pub fn parse(input: &[u8]) -> nom::IResult<&[u8], Self> {
        let (input, bits) = nom::number::complete::le_u32(input)?;
        Ok((input, Flags::from_bits_truncate(bits)))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, NomLE)]
#[repr(u8)]
pub enum EncryptionAlgorithm {
    Invalid = 0,
    Aes256Xts = 1,
    Aes256Gcm = 2,
    Aes256Cbc = 3,
}
