use bitflags::bitflags;
use nom::IResult;
use nom::number::complete::{le_u16, le_u32};

#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Linux2 {
    pub l_i_blocks_high: u16,
    pub l_i_file_acl_high: u16,
    pub l_i_uid_high: u16,
    pub l_i_gid_high: u16,
    pub l_i_checksum_lo: u16,
    pub l_i_reserved: u16,
}

impl Linux2 {
    pub fn parse(input: &[u8]) -> IResult<&[u8], Self> {
        let (input, l_i_blocks_high) = le_u16(input)?;
        let (input, l_i_file_acl_high) = le_u16(input)?;
        let (input, l_i_uid_high) = le_u16(input)?;
        let (input, l_i_gid_high) = le_u16(input)?;
        let (input, l_i_checksum_lo) = le_u16(input)?;
        let (input, l_i_reserved) = le_u16(input)?;

        Ok((
            input,
            Linux2 {
                l_i_blocks_high,
                l_i_file_acl_high,
                l_i_uid_high,
                l_i_gid_high,
                l_i_checksum_lo,
                l_i_reserved,
            },
        ))
    }
}

bitflags! {
    #[derive(Debug, PartialEq, Eq, Clone, Copy)]
    pub struct InodeFileType: u16 {
        const S_IFIFO = 0x1000;
        const S_IFCHR = 0x2000;
        const S_IFDIR = 0x4000;
        const S_IFBLK = 0x6000;
        const S_IFREG = 0x8000;
        const S_IFSOCK = 0xC000;
        const S_IFLNK = 0xA000;
    }
}

bitflags! {
    #[derive(Debug, PartialEq, Eq, Clone, Copy)]
    pub struct InodePerm: u16 {
        const S_IREAD = 0x0100;
        const S_IWRITE = 0x0080;
        const S_IEXEC = 0x0040;
        const S_ISUID = 0x0800;
        const S_ISGID = 0x0400;
    }
}

#[repr(C)]
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Inode {
    pub mode: u16,
    pub uid: u16,
    pub size: u32,
    pub atime: u32,
    pub ctime: u32,
    pub mtime: u32,
    pub dtime: u32,
    pub gid: u16,
    pub links_count: u16,
    pub blocks: u32,
    pub flags: u32,
    pub osd1: u32,
    pub block: [u32; 15],
    pub generation: u32,
    pub file_acl: u32,
    pub size_hi: u32,
    pub faddr: u32,
    pub osd2: Linux2,
    pub i_extra_isize: u16,
    pub i_checksum_hi: u16,
    pub i_ctime_extra: u32,
    pub i_mtime_extra: u32,
    pub i_atime_extra: u32,
    pub i_crtime: u32,
    pub i_crtime_extra: u32,
    pub i_version_hi: u32,
}

impl Inode {
    pub const ROOT_INODE: u32 = 2;
    pub const JOURNAL_INODE: u32 = 8;
    pub const UNDEL_DIR_INODE: u32 = 6;
    pub const LOST_AND_FOUND_INODE: u32 = 11;
    pub const EXT4_INODE_MODE_FILE: u16 = 0x8000;
    pub const EXT4_INODE_MODE_DIR: u16 = 0x4000;
    pub const EXT4_INODE_MODE_SYMLINK: u16 = 0xA000;
    pub const EXT4_INODE_MODE_TYPE_MASK: u16 = 0xF000;
    pub const EXT4_INODE_MODE_PERM_MASK: u16 = 0x0FFF;
    pub const EXT4_INODE_BLOCK_SIZE: usize = 512;
    pub const EXT4_GOOD_OLD_INODE_SIZE: u16 = 128;
    pub const EXT4_INODE_FLAG_EXTENTS: u32 = 0x00080000;

    pub fn parse(input: &[u8]) -> IResult<&[u8], Self> {
        let (input, mode) = le_u16(input)?;
        let (input, uid) = le_u16(input)?;
        let (input, size) = le_u32(input)?;
        let (input, atime) = le_u32(input)?;
        let (input, ctime) = le_u32(input)?;
        let (input, mtime) = le_u32(input)?;
        let (input, dtime) = le_u32(input)?;
        let (input, gid) = le_u16(input)?;
        let (input, links_count) = le_u16(input)?;
        let (input, blocks) = le_u32(input)?;
        let (input, flags) = le_u32(input)?;
        let (input, osd1) = le_u32(input)?;
        let (input, block_vec) = nom::multi::count(le_u32, 15)(input)?;
        let (input, generation) = le_u32(input)?;
        let (input, file_acl) = le_u32(input)?;
        let (input, size_hi) = le_u32(input)?;
        let (input, faddr) = le_u32(input)?;
        let (input, osd2) = Linux2::parse(input)?;
        let (input, i_extra_isize) = le_u16(input)?;
        let (input, i_checksum_hi) = le_u16(input)?;
        let (input, i_ctime_extra) = le_u32(input)?;
        let (input, i_mtime_extra) = le_u32(input)?;
        let (input, i_atime_extra) = le_u32(input)?;
        let (input, i_crtime) = le_u32(input)?;
        let (input, i_crtime_extra) = le_u32(input)?;
        let (input, i_version_hi) = le_u32(input)?;

        let mut block_arr = [0u32; 15];
        block_arr.copy_from_slice(&block_vec);

        Ok((
            input,
            Inode {
                mode,
                uid,
                size,
                atime,
                ctime,
                mtime,
                dtime,
                gid,
                links_count,
                blocks,
                flags,
                osd1,
                block: block_arr,
                generation,
                file_acl,
                size_hi,
                faddr,
                osd2,
                i_extra_isize,
                i_checksum_hi,
                i_ctime_extra,
                i_mtime_extra,
                i_atime_extra,
                i_crtime,
                i_crtime_extra,
                i_version_hi,
            },
        ))
    }

    pub fn is_directory(&self) -> bool {
        (self.mode & Self::EXT4_INODE_MODE_TYPE_MASK) == Self::EXT4_INODE_MODE_DIR
    }

    pub fn is_regular_file(&self) -> bool {
        (self.mode & Self::EXT4_INODE_MODE_TYPE_MASK) == Self::EXT4_INODE_MODE_FILE
    }

    pub fn is_symlink(&self) -> bool {
        (self.mode & Self::EXT4_INODE_MODE_TYPE_MASK) == Self::EXT4_INODE_MODE_SYMLINK
    }

    pub fn uses_extents(&self) -> bool {
        (self.flags & Self::EXT4_INODE_FLAG_EXTENTS) != 0
    }
}
