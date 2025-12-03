use bitflags::bitflags;

#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Linux2 {
    pub l_i_blocks_high: u16,   // Higher 16 bits of allocated blocks count
    pub l_i_file_acl_high: u16, // Higher 16 bits of file ACL
    pub l_i_uid_high: u16,      // Higher 16 bits of user ID
    pub l_i_gid_high: u16,      // Higher 16 bits of group ID
    pub l_i_checksum_lo: u16,   // Lower checksum
    pub l_i_reserved: u16,      // Reserved field
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
    pub mode: u16,        // File type and permissions
    pub uid: u16,         // Owner user ID
    pub size: u32,        // Lower 32 bits of file size
    pub atime: u32,       // Last access time
    pub ctime: u32,       // Creation time
    pub mtime: u32,       // Last modification time
    pub dtime: u32,       // Deletion time
    pub gid: u16,         // Owner group ID
    pub links_count: u16, // Link count
    pub blocks: u32,      // Allocated blocks count
    pub flags: u32,       // File flags
    pub osd1: u32,        // OS-dependent field 1
    pub block: [u32; 15], // Data block pointers
    pub generation: u32,  // File version (NFS)
    pub file_acl: u32,    // File ACL
    pub size_hi: u32,     // Higher 32 bits of file size
    pub faddr: u32,       // Deprecated fragment address
    pub osd2: Linux2,     // OS-dependent field 2

    pub i_extra_isize: u16,  // Extra inode size
    pub i_checksum_hi: u16,  // High checksum (crc32c(uuid+inum+inode) BE)
    pub i_ctime_extra: u32,  // Extra creation time (nanosec << 2 | epoch)
    pub i_mtime_extra: u32,  // Extra modification time (nanosec << 2 | epoch)
    pub i_atime_extra: u32,  // Extra access time (nanosec << 2 | epoch)
    pub i_crtime: u32,       // Creation time
    pub i_crtime_extra: u32, // Extra creation time (nanosec << 2 | epoch)
    pub i_version_hi: u32,   // Higher 32 bits of version
}

impl Inode {
    pub const ROOT_INODE: u32 = 2; // Root directory inode
    pub const JOURNAL_INODE: u32 = 8; // Journal file inode
    pub const UNDEL_DIR_INODE: u32 = 6; // Undelete directory inode
    pub const LOST_AND_FOUND_INODE: u32 = 11; // lost+found directory inode
    pub const EXT4_INODE_MODE_FILE: usize = 0x8000;
    pub const EXT4_INODE_MODE_TYPE_MASK: u16 = 0xF000;
    pub const EXT4_INODE_MODE_PERM_MASK: u16 = 0x0FFF;
    pub const EXT4_INODE_BLOCK_SIZE: usize = 512;
    pub const EXT4_GOOD_OLD_INODE_SIZE: u16 = 128;
    pub const EXT4_INODE_FLAG_EXTENTS: usize = 0x00080000; /* Inode uses extents */
}
