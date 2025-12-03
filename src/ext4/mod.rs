mod block;
mod extent;
mod inode;
mod superblock;

pub const BLOCK_SIZE: usize = 0x1000; // 4KB
pub const SECTORS_PER_BLOCK: usize = BLOCK_SIZE / 512;

pub type Ext4Lblk = u32;
pub type Ext4Fsblk = u64;

pub const EOK: usize = 0;

/// File
pub const EXT4_MAX_FILE_SIZE: u64 = 16 * 1024 * 1024 * 1024; // 16TB

/// libc file open flags
pub const O_ACCMODE: i32 = 0o0003;
pub enum OpenFlags {
    ReadOnly = 0,
    WriteOnly = 1,
    ReadWrite = 2,
    Create = 0o100,
    Exclusive = 0o200,
    NoCTTY = 0o400,
    Truncate = 0o1000,
    Append = 0o2000,
    NonBlocking = 0o4000,
    Sync = 0o4010000,
    Async = 0o20000,
    LargeFile = 0o100000,
    Directory = 0o200000,
    NoFollow = 0o400000,
    CloExec = 0o2000000,
    Direct = 0o40000,
    NoAtime = 0o1000000,
    Path = 0o10000000,
    DSync = 0o10000,
}

/// linux access syscall flags
pub enum AccessMode {
    Exist = 0b000,   // Test for existence of file
    Execute = 0b001, // Test for execute or search permission
    Write = 0b010,   // Test for write permission
    Read = 0b100,    // Test for read permission
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct DirectoryEntry {
    pub inode: u32,                    // Inode number this entry points to
    pub entry_len: u16,                // Distance to the next directory entry
    pub name_len: u8,                  // Lower 8 bits of name length
    pub inner: DirectoryEntryInternal, // Union member
    pub name: [u8; 255],               // File name
}

/// Internal directory entry structure.
#[repr(C)]
#[derive(Clone, Copy)]
pub union DirectoryEntryInternal {
    pub name_length_high: u8, // Higher 8 bits of name length
    pub inode_type: u8,       // Type of the referenced inode (in rev >= 0.5)
}

/// Fake directory entry structure. Used for directory entry iteration.
#[repr(C)]
pub struct FakeDirectoryEntry {
    inode: u32,
    entry_length: u16,
    name_length: u8,
    inode_type: u8,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct DirectoryEntryTail {
    pub reserved_zero1: u32,
    pub rec_len: u16,
    pub reserved_zero2: u8,
    pub reserved_ft: u8,
    pub checksum: u32, // crc32c(uuid+inum+dirblock)
}

pub struct DirectorySearchResult {
    pub dentry: DirectoryEntry,
    pub pblock_id: usize,   // disk block id
    pub offset: usize,      // offset in block
    pub prev_offset: usize, //prev direntry offset
}

bitflags::bitflags! {
    #[derive(PartialEq, Eq)]
    pub struct DirEntryType: u8 {
        const EXT4_DE_UNKNOWN = 0;
        const EXT4_DE_REG_FILE = 1;
        const EXT4_DE_DIR = 2;
        const EXT4_DE_CHRDEV = 3;
        const EXT4_DE_BLKDEV = 4;
        const EXT4_DE_FIFO = 5;
        const EXT4_DE_SOCK = 6;
        const EXT4_DE_SYMLINK = 7;
    }
}
