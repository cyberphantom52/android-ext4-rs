use nom::Finish;
use nom_derive::{NomLE, Parse};

use crate::{Ext4Error, Result};

#[derive(Debug, Clone, Copy, NomLE)]
#[repr(C)]
struct XAttrHeader {
    #[nom(Verify(*magic == 0xEA020000))]
    magic: u32,
    refcount: u32,
    blocks: u32,
    hash: u32,
    checksum: u32,
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
pub(crate) struct XAttrIbodyHeader {
    #[nom(Verify(*magic == 0xEA020000))]
    magic: u32,
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
struct XAttrEntryHeader {
    name_len: u8,
    name_index: XAttrNameIndex,
    value_offs: u16,
    value_inum: u32,
    value_size: u32,
    hash: u32,
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
    header: XAttrEntryHeader,
    name: String,
    value: Option<Vec<u8>>,
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

    /// Parse xattr entries from raw data
    ///
    /// # Arguments
    /// * `data` - The full data buffer (including any header)
    /// * `entries_start` - Offset where entries begin (after header)
    /// * `value_base` - Base offset for e_value_offs calculation
    ///   - For blocks: 0 (e_value_offs is relative to block start)
    ///   - For inline: entries_start (e_value_offs is relative to first entry)
    pub fn parse(data: &[u8], entries_start: usize, value_base: usize) -> Result<Vec<XAttrEntry>> {
        let mut xattrs = Vec::new();
        let mut pos = entries_start;

        while pos + Self::HEADER_SIZE <= data.len() {
            let entry_data = &data[pos..];
            let header = XAttrEntryHeader::parse(entry_data)?;

            if header.is_end_of_entries() {
                break;
            }

            let name = entry_data
                .get(Self::HEADER_SIZE..Self::HEADER_SIZE + header.name_len as usize)
                .map(String::from_utf8_lossy)
                .ok_or_else(|| Ext4Error::Parse("XAttrEntry: name out of bounds".to_string()))?
                .to_string();

            let mut value = None;
            if header.value_inum == 0 && header.value_size > 0 {
                // e_value_offs + value_base gives absolute offset in data
                let value_start = header.value_offs as usize + value_base;
                let value_end = value_start + header.value_size as usize;
                value = data.get(value_start..value_end).map(|v| v.to_vec());
            }

            let entry = XAttrEntry {
                header,
                name,
                value,
            };
            pos += entry.size();
            xattrs.push(entry);
        }

        Ok(xattrs)
    }

    /// Get the full size of this entry (header + name, aligned to 4 bytes)
    pub fn size(&self) -> usize {
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
        if !self.is_selinux() {
            return None;
        }

        let trimmed = self
            .value
            .as_deref()
            .map(|v| v.strip_suffix(&[0]))
            .unwrap_or(self.value.as_deref())?;

        return Some(String::from_utf8_lossy(trimmed).to_string());
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

        if !self.is_capability() || self.value.is_none() {
            return None;
        }

        let cap = VfsCapData::parse(self.value.as_deref().unwrap()).ok()?;

        cap.capabilities()
            .map(|caps| format!(" capabilities={:#x}", caps))
    }
}

pub fn parse_xattrs_from_block(block_data: &[u8]) -> Result<Vec<XAttrEntry>> {
    XAttrHeader::parse(block_data)?; // Validate magic

    // Entries start after header (offset 32)
    // e_value_offs is relative to block start (offset 0)
    XAttrEntry::parse(
        block_data,
        XAttrHeader::SIZE, // entries_start = 32
        0,                 // value_base = 0 (relative to block start)
    )
}
