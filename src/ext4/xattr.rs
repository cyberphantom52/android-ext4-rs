use nom::Finish;
use nom_derive::{NomLE, Parse};

use crate::{Ext4Error, Result};

#[derive(Debug, Clone, Copy, NomLE)]
#[repr(C)]
pub struct XAttrHeader {
    #[nom(Verify(*magic == 0xEA020000))]
    pub magic: u32,
    pub refcount: u32,
    pub blocks: u32,
    pub hash: u32,
    pub checksum: u32,
    _reserved: [u8; 12],
}

impl XAttrHeader {
    pub const SIZE: usize = 32;

    pub fn parse(bytes: &[u8]) -> Result<Self> {
        match Parse::parse(bytes).finish() {
            Ok((_, descriptor)) => Ok(descriptor),
            Err(e) => Err(Ext4Error::Parse(format!("{:?}", e))),
        }
    }
}

#[derive(Debug, Clone, Copy, NomLE)]
#[repr(C)]
pub struct XAttrIbodyHeader {
    #[nom(Verify(*magic == 0xEA020000))]
    pub magic: u32,
}

impl XAttrIbodyHeader {
    pub const SIZE: usize = 4;

    pub fn parse(bytes: &[u8]) -> Result<Self> {
        match Parse::parse(bytes).finish() {
            Ok((_, descriptor)) => Ok(descriptor),
            Err(e) => Err(Ext4Error::Parse(format!("{:?}", e))),
        }
    }
}

#[derive(Debug, Clone, NomLE)]
#[repr(C)]
pub struct XAttrEntryHeader {
    pub name_len: u8,
    pub name_index: XAttrNameIndex,
    pub value_offs: u16,
    pub value_inum: u32,
    pub value_size: u32,
    pub hash: u32,
}

impl XAttrEntryHeader {
    pub fn parse(bytes: &[u8]) -> Result<Self> {
        match Parse::parse(bytes).finish() {
            Ok((_, descriptor)) => Ok(descriptor),
            Err(e) => Err(Ext4Error::Parse(format!("{:?}", e))),
        }
    }

    pub fn is_end_of_entries(&self) -> bool {
        (self.name_len as u32 | self.name_index as u32 | self.value_offs as u32 | self.value_inum)
            == 0
    }
}

#[derive(Debug, Clone)]
pub struct XAttrEntry {
    pub header: XAttrEntryHeader,
    pub name: String,
    pub value: Vec<u8>,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, NomLE)]
#[repr(u8)]
pub enum XAttrNameIndex {
    NoPrefix = 0,
    User = 1,
    SystemPosixAclAccess = 2,
    SystemPosixAclDefault = 3,
    Trusted = 4,
    Security = 6,
    System = 7,
    SystemRichAcl = 8,
}

impl XAttrNameIndex {
    pub fn is_acl(&self) -> bool {
        matches!(
            self,
            XAttrNameIndex::SystemPosixAclAccess
                | XAttrNameIndex::SystemPosixAclDefault
                | XAttrNameIndex::SystemRichAcl
        )
    }
}

impl std::fmt::Display for XAttrNameIndex {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            XAttrNameIndex::NoPrefix => "",
            XAttrNameIndex::User => "user.",
            XAttrNameIndex::SystemPosixAclAccess => "system.posix_acl_access",
            XAttrNameIndex::SystemPosixAclDefault => "system.posix_acl_default",
            XAttrNameIndex::Trusted => "trusted.",
            XAttrNameIndex::Security => "security.",
            XAttrNameIndex::System => "system.",
            XAttrNameIndex::SystemRichAcl => "system.richacl",
        };
        write!(f, "{}", s)
    }
}

impl XAttrEntry {
    pub const HEADER_SIZE: usize = 16; // Size without the name

    /// Get the full size of this entry (header + name, aligned to 4 bytes)
    pub fn entry_size(&self) -> usize {
        let size = Self::HEADER_SIZE + self.name.len();
        // Align to 4 bytes
        (size + 3) & !3
    }

    /// Get the full attribute name including the prefix
    pub fn full_name(&self) -> String {
        if self.header.name_index.is_acl() {
            format!("{}", self.header.name_index)
        } else {
            format!("{}{}", self.header.name_index, self.name)
        }
    }

    /// Check if this is a SELinux context attribute
    pub fn is_selinux(&self) -> bool {
        self.full_name() == "security.selinux"
    }

    /// Check if this is a capability attribute
    pub fn is_capability(&self) -> bool {
        self.full_name() == "security.capability"
    }

    /// Get the SELinux context as a string (without null terminator)
    pub fn selinux_context(&self) -> Option<String> {
        if self.is_selinux() {
            // SELinux context is null-terminated
            let value = if self.value.last() == Some(&0) {
                &self.value[..self.value.len() - 1]
            } else {
                &self.value
            };
            Some(String::from_utf8_lossy(value).to_string())
        } else {
            None
        }
    }

    /// Parse capability value and return it as a hex string
    /// Format: capabilities=0x... or empty if no caps
    pub fn capability_string(&self) -> Option<String> {
        #[derive(Debug, NomLE)]
        struct CapData {
            permitted: u32,
            _inheritable: u32,
        }

        #[derive(Debug, NomLE)]
        struct VfsCapData {
            _magic_etc: u32,
            #[nom(Count = "2")] // VFS_CAP_U32
            data: Vec<CapData>,
        }

        impl VfsCapData {
            pub fn parse(bytes: &[u8]) -> Result<Self> {
                match Parse::parse(bytes).finish() {
                    Ok((_, descriptor)) => Ok(descriptor),
                    Err(e) => Err(Ext4Error::Parse(format!("{:?}", e))),
                }
            }

            pub fn capabilities(&self) -> Option<u64> {
                let permitted_lo = self.data.get(0)?.permitted;
                let permitted_hi = self.data.get(1)?.permitted;

                let caps = ((permitted_hi as u64) << 32) | (permitted_lo as u64);

                if caps == 0 { None } else { Some(caps) }
            }
        }

        if !self.is_capability() || self.value.len() < 20 {
            return None;
        }

        let cap = VfsCapData::parse(&self.value).ok()?;
        cap.capabilities()
            .map(|caps| format!(" capabilities={:#x}", caps))
    }
}

fn parse_xattrs(raw_data: &[u8], value_offset_adjustment: isize) -> Result<Vec<XAttrEntry>> {
    let mut xattrs = Vec::new();
    let mut pos = 0;

    while pos + XAttrEntry::HEADER_SIZE <= raw_data.len() {
        let header = XAttrEntryHeader::parse(&raw_data[pos..])?;

        if header.is_end_of_entries() {
            break;
        }

        let name = raw_data
            .get(
                pos + XAttrEntry::HEADER_SIZE
                    ..pos + XAttrEntry::HEADER_SIZE + header.name_len as usize,
            )
            .map(String::from_utf8_lossy)
            .ok_or_else(|| Ext4Error::Parse("XAttrEntry: name out of bounds".to_string()))?
            .to_string();

        let value = if header.value_inum != 0 {
            // External xattr (stored in another inode) - not supported yet
            Vec::new()
        } else if header.value_size > 0 {
            // Internal xattr - value is in the same data block
            // Calculate actual offset: e_value_offs + adjustment
            let actual_offset = (header.value_offs as isize + value_offset_adjustment) as usize;
            let value_end = actual_offset + header.value_size as usize;

            if actual_offset < raw_data.len() && value_end <= raw_data.len() {
                raw_data[actual_offset..value_end].to_vec()
            } else {
                Vec::new()
            }
        } else {
            Vec::new()
        };

        let entry = XAttrEntry {
            header,
            name,
            value,
        };

        let entry_size = entry.entry_size();
        xattrs.push(entry);
        pos += entry_size;
    }

    Ok(xattrs)
}

/// Parse extended attributes from a block
pub fn parse_xattrs_from_block(block_data: &[u8]) -> Result<Vec<XAttrEntry>> {
    if block_data.len() < XAttrHeader::SIZE {
        return Ok(Vec::new());
    }

    // Parse and validate header
    let _header = XAttrHeader::parse(block_data)?;

    // Entries start after the header, aligned to 4 bytes
    let entries_start = (XAttrHeader::SIZE + 3) & !3;

    if entries_start >= block_data.len() {
        return Ok(Vec::new());
    }

    // For block xattrs, value_offs is relative to the start of the block
    // So we pass negative entries_start as adjustment
    parse_xattrs(&block_data[entries_start..], -(entries_start as isize))
}
