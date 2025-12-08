use bitflags::bitflags;
use nom::Finish;
use nom_derive::{NomLE, Parse};

use crate::{
    Ext4Error, Result,
    ext4::xattr::{XAttrEntry, XAttrEntryHeader, XAttrIbodyHeader},
};

#[repr(C)]
#[derive(Debug, Clone, NomLE)]
pub struct Inode {
    #[nom(Parse = "Mode::parse")]
    pub mode: Mode,
    pub uid: u16,
    pub size: u32,
    pub atime: u32,
    pub ctime: u32,
    pub mtime: u32,
    pub dtime: u32,
    pub gid: u16,
    pub links_count: u16,
    pub blocks: u32,

    #[nom(Parse = "Flags::parse")]
    pub flags: Flags,

    pub osd1: u32,

    pub block: [u32; 15],
    pub generation: u32,
    pub file_acl: u32,
    pub size_hi: u32,
    pub faddr: u32,

    pub osd2: Linux2,

    pub extra_isize: u16,
    pub checksum_hi: u16,
    pub ctime_extra: u32,
    pub mtime_extra: u32,
    pub atime_extra: u32,
    pub crtime: u32,
    pub crtime_extra: u32,
    pub version_hi: u32,
    pub project_id: u32,

    #[nom(Ignore)]
    pub inline_xattrs: Vec<XAttrEntry>,
}

impl Inode {
    // Special inode numbers
    pub const ROOT_INODE: u32 = 2;
    const _JOURNAL_INODE: u32 = 8;
    const _UNDEL_DIR_INODE: u32 = 6;
    const _LOST_AND_FOUND_INODE: u32 = 11;

    // Mode masks
    const MODE_PERM_MASK: u16 = 0x0FFF;
    const MODE_TYPE_MASK: u16 = 0xF000;

    // Other constants
    const _BLOCK_SIZE: usize = 512;
    pub const GOOD_OLD_SIZE: u16 = 128;
    pub const DIRECT_BLOCKS: u32 = 12;
    pub const INDIRECT_BLOCK_IDX: usize = 12;
    pub const DOUBLE_INDIRECT_BLOCK_IDX: usize = 13;
    pub const TRIPLE_INDIRECT_BLOCK_IDX: usize = 14;
    pub const FAST_SYMLINK_MAX_SIZE: u64 = 60;

    pub fn parse(bytes: &[u8]) -> Result<Self> {
        let mut inode: Inode = match Parse::parse(bytes).finish() {
            Ok((_, descriptor)) => descriptor,
            Err(e) => return Err(Ext4Error::Parse(format!("{:?}", e))),
        };

        if inode.extra_isize > 0 {
            let start_offset = (Self::GOOD_OLD_SIZE + inode.extra_isize) as usize;
            let inode_size = bytes.len();

            let inline_data = bytes.get(start_offset..inode_size).ok_or_else(|| {
                Ext4Error::Parse("Inode extra size exceeds available data".to_string())
            })?;

            if inline_data.len() >= 4 {
                XAttrIbodyHeader::parse(inline_data)?; // Validate header
                if inline_data.len() > XAttrIbodyHeader::SIZE {
                    inode.inline_xattrs =
                        Self::parse_inline_xattr_entries(&inline_data[XAttrIbodyHeader::SIZE..]);
                }
            }
        }

        Ok(inode)
    }
    fn parse_inline_xattr_entries(raw_data: &[u8]) -> Vec<XAttrEntry> {
        let mut xattrs = Vec::new();
        let mut pos = 0;

        while pos + XAttrEntry::HEADER_SIZE <= raw_data.len() {
            let start_offset = pos + XAttrEntry::HEADER_SIZE;
            let header = match XAttrEntryHeader::parse(&raw_data[pos..]) {
                Ok(h) => h,
                Err(_) => break,
            };

            if header.is_end_of_entries() {
                break;
            }

            let name = match raw_data.get(start_offset..start_offset + header.name_len as usize) {
                Some(bytes) => String::from_utf8_lossy(bytes).to_string(),
                None => break,
            };

            let mut value = Vec::new();
            if header.value_inum == 0 && header.value_size > 0 {
                let value_start_offset = header.value_offs as usize;
                let value_end_offset = value_start_offset + header.value_size as usize;
                value = raw_data
                    .get(value_start_offset..value_end_offset)
                    .map(|v| v.to_vec())
                    .unwrap_or_default();
            }

            let entry = XAttrEntry {
                header,
                name,
                value,
            };

            pos += entry.entry_size();
            xattrs.push(entry);
        }

        xattrs
    }

    /// Get the file type from the inode
    pub fn file_type(&self) -> Option<FileType> {
        FileType::from_mode(self.mode.bits())
    }

    /// Check if this is a directory
    pub fn is_directory(&self) -> bool {
        matches!(self.file_type(), Some(FileType::Directory))
    }

    /// Check if this is a regular file
    pub fn is_regular_file(&self) -> bool {
        matches!(self.file_type(), Some(FileType::RegularFile))
    }

    /// Check if this is a symbolic link
    pub fn is_symlink(&self) -> bool {
        matches!(self.file_type(), Some(FileType::SymbolicLink))
    }

    /// Check if this inode uses extents
    pub fn uses_extents(&self) -> bool {
        self.flags.contains(Flags::Extents)
    }

    /// Get only the permission bits from the mode
    pub fn permissions(&self) -> Mode {
        Mode::from_bits_truncate(self.mode.bits() & Self::MODE_PERM_MASK)
    }

    pub fn xattr_block_number(&self) -> u64 {
        (self.osd2.file_acl_high as u64) << 32 | self.file_acl as u64
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u16)]
pub enum FileType {
    Fifo = 0x1000,
    CharacterDevice = 0x2000,
    Directory = 0x4000,
    BlockDevice = 0x6000,
    RegularFile = 0x8000,
    SymbolicLink = 0xA000,
    Socket = 0xC000,
}

impl FileType {
    pub fn from_mode(mode: u16) -> Option<Self> {
        match mode & Inode::MODE_TYPE_MASK {
            0x1000 => Some(Self::Fifo),
            0x2000 => Some(Self::CharacterDevice),
            0x4000 => Some(Self::Directory),
            0x6000 => Some(Self::BlockDevice),
            0x8000 => Some(Self::RegularFile),
            0xA000 => Some(Self::SymbolicLink),
            0xC000 => Some(Self::Socket),
            _ => None,
        }
    }
}

bitflags! {
    #[derive(Debug, PartialEq, Eq, Clone, Copy)]
    pub struct Mode: u16 {
        // Permission bits
        const OtherExecute = 0x001;
        const OtherWrite = 0x002;
        const OtherRead = 0x004;
        const GroupExecute = 0x008;
        const GroupWrite = 0x010;
        const GroupRead = 0x020;
        const OwnerExecute = 0x040;
        const OwnerWrite = 0x080;
        const OwnerRead = 0x100;

        // Special bits
        const StickyBit = 0x200;
        const SetGid = 0x400;
        const SetUid = 0x800;

        // File type bits (mutually exclusive, use FileType enum to decode)
        const TypeFifo = 0x1000;            // S_IFIFO
        const TypeCharacterDevice = 0x2000; // S_IFCHR
        const TypeDirectory = 0x4000;       // S_IFDIR
        const TypeBlockDevice = 0x6000;     // S_IFBLK
        const TypeRegularFile = 0x8000;     // S_IFREG
        const TypeSymbolicLink = 0xA000;    // S_IFLNK
        const TypeSocket = 0xC000;          // S_IFSOCK
    }
}

impl Mode {
    pub fn parse(input: &[u8]) -> nom::IResult<&[u8], Self> {
        let (input, bits) = nom::number::complete::le_u16(input)?;
        Ok((input, Self::from_bits_truncate(bits)))
    }
}

bitflags! {
    #[derive(Debug, PartialEq, Eq, Clone, Copy)]
    pub struct Flags: u32 {
        const SecureDelete = 0x00000001;
        const Undelete = 0x00000002;
        const Compressed = 0x00000004;
        const Synchronous = 0x00000008;
        const Immutable = 0x00000010;
        const AppendOnly = 0x00000020;
        const NoDump = 0x00000040;
        const NoAccessTime = 0x00000080;
        const DirtyCompressed = 0x00000100;
        const CompressedClusters = 0x00000200;
        const NoCompress = 0x00000400;
        const Encrypted = 0x00000800;
        const HashedIndex = 0x00001000;
        const AfsDirectory = 0x00002000;
        const JournalData = 0x00004000;
        const NoTailMerge = 0x00008000;
        const DirectorySync = 0x00010000;
        const TopDirectory = 0x00020000;
        const HugeFile = 0x00040000;
        const Extents = 0x00080000;
        const VerityProtected = 0x00100000;
        const ExtendedAttribute = 0x00200000;
        const ExtentsOverflow = 0x00400000;
        const Snapshot = 0x01000000;
        const SnapshotDeleted = 0x04000000;
        const SnapshotShrunk = 0x08000000;
        const InlineData = 0x10000000;
        const ProjectInherit = 0x20000000;
        const Reserved = 0x80000000;
    }
}

impl Flags {
    pub const USER_VISIBLE: u32 = 0x705BDFFF;
    pub const USER_MODIFIABLE: u32 = 0x604BC0FF;

    pub fn parse(input: &[u8]) -> nom::IResult<&[u8], Self> {
        let (input, bits) = nom::number::complete::le_u32(input)?;
        Ok((input, Self::from_bits_truncate(bits)))
    }

    /// Returns only the user-visible flags
    pub fn user_visible(&self) -> Self {
        Self::from_bits_truncate(self.bits() & Self::USER_VISIBLE)
    }

    /// Returns only the user-modifiable flags
    pub fn user_modifiable(&self) -> Self {
        Self::from_bits_truncate(self.bits() & Self::USER_MODIFIABLE)
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, NomLE)]
pub struct Linux2 {
    pub blocks_high: u16,
    pub file_acl_high: u16,
    pub uid_high: u16,
    pub gid_high: u16,
    pub checksum_lo: u16,
    pub reserved: u16,
}

impl Linux2 {
    pub fn parse(bytes: &[u8]) -> Result<Self> {
        match Parse::parse(bytes).finish() {
            Ok((_, descriptor)) => Ok(descriptor),
            Err(e) => Err(Ext4Error::Parse(format!("{:?}", e))),
        }
    }
}
