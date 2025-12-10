use std::io::{Read, Seek};
use std::path::{Path, PathBuf};

use crate::{
    DirectoryWalker, Error, Result, Volume,
    ext4::{DirectoryEntry, InodeReader, inode::Inode},
};

/// Represents a directory in the ext4 filesystem
pub struct Directory<R: Read + Seek, F: Fn() -> R> {
    pub(crate) volume: Volume<R, F>,
    path: PathBuf,
    inode: Inode,
    entries: Vec<DirectoryEntry>,
}

impl<R: Read + Seek, F: Fn() -> R> Directory<R, F> {
    /// Create a new Directory from a volume, inode, and path
    pub(crate) fn new(
        volume: &Volume<R, F>,
        inode: Inode,
        path: impl Into<PathBuf>,
    ) -> Result<Self> {
        let path = path.into();
        if !inode.is_directory() {
            return Err(Error::NotADirectory(format!("{}", path.display())));
        }

        let entries = Self::parse_entries(volume, &inode)?;

        Ok(Self {
            volume: volume.clone(),
            path,
            inode,
            entries,
        })
    }

    /// Get a reference to the inode
    pub fn inode(&self) -> &Inode {
        &self.inode
    }

    /// Get the path of this directory
    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn entries(&self) -> &[DirectoryEntry] {
        &self.entries
    }

    pub fn entries_mut(&mut self) -> &mut Vec<DirectoryEntry> {
        &mut self.entries
    }

    /// Parse directory entries from raw data
    fn parse_entries(volume: &Volume<R, F>, inode: &Inode) -> Result<Vec<DirectoryEntry>> {
        let data = InodeReader::new(volume).read_all(inode)?;
        let mut entries = Vec::new();
        let mut offset = 0;

        while offset < data.len() {
            if offset + DirectoryEntry::HEADER_SIZE > data.len() {
                break;
            }

            let inode_num = u32::from_le_bytes(
                data[offset..offset + 4]
                    .try_into()
                    .map_err(|_| Error::CorruptedDirectoryEntry(offset))?,
            );

            let entry_len = u16::from_le_bytes([data[offset + 4], data[offset + 5]]);

            if entry_len == 0 || entry_len as usize > data.len() - offset {
                break;
            }

            if inode_num != 0 {
                let name_len = data[offset + 6];
                let inode_type = data[offset + 7];

                let mut name = [0u8; DirectoryEntry::MAX_NAME_LEN];
                let actual_name_len = DirectoryEntry::MAX_NAME_LEN.min(name_len as usize);
                if offset + DirectoryEntry::HEADER_SIZE + actual_name_len <= data.len() {
                    name[..actual_name_len].copy_from_slice(
                        &data[offset + DirectoryEntry::HEADER_SIZE
                            ..offset + DirectoryEntry::HEADER_SIZE + actual_name_len],
                    );
                }

                entries.push(DirectoryEntry {
                    inode: inode_num,
                    entry_len,
                    name_len,
                    inode_type,
                    name,
                });
            }

            offset += entry_len as usize;
        }

        Ok(entries)
    }

    /// Create a walker for recursive directory traversal
    pub fn walk(self) -> DirectoryWalker<R, F> {
        DirectoryWalker::new(self)
    }

    pub fn find(&self, name: &str) -> Option<&DirectoryEntry> {
        self.entries()
            .into_iter()
            .find(|entry| entry.name_str() == name)
    }
}

impl<R: Read + Seek, F: Fn() -> R> IntoIterator for Directory<R, F> {
    type Item = DirectoryEntry;
    type IntoIter = std::vec::IntoIter<DirectoryEntry>;

    fn into_iter(self) -> Self::IntoIter {
        self.entries.into_iter()
    }
}
