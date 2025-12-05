use std::io::{Read, Seek};

use crate::{
    DirectoryWalker,
    ext4::{DirectoryEntry, Ext4Error, Inode, Result, Volume},
};

/// Represents a directory in the ext4 filesystem
pub struct Directory<'a, R: Read + Seek> {
    pub(crate) volume: &'a mut Volume<R>,
    inode: Inode,
    entries: Vec<DirectoryEntry>,
}

impl<'a, R: Read + Seek> Directory<'a, R> {
    /// Create a new Directory from a volume and inode
    pub(crate) fn new(volume: &'a mut Volume<R>, inode: Inode) -> Result<Self> {
        if !inode.is_directory() {
            return Err(Ext4Error::NotADirectory);
        }

        let entries = volume.read_directory_entries(&inode)?;

        Ok(Self {
            volume,
            inode,
            entries,
        })
    }

    /// Get a reference to the inode
    pub fn inode(&self) -> &Inode {
        &self.inode
    }

    /// Get all entries
    pub fn entries(&self) -> &[DirectoryEntry] {
        &self.entries
    }

    /// Create a walker for recursive directory traversal
    pub fn walk(self) -> DirectoryWalker<'a, R> {
        DirectoryWalker::new(self)
    }

    pub fn find(&self, name: &str) -> Option<&DirectoryEntry> {
        self.entries.iter().find(|entry| entry.name_str() == name)
    }
}

impl<'a, R: Read + Seek> IntoIterator for Directory<'a, R> {
    type Item = DirectoryEntry;
    type IntoIter = std::vec::IntoIter<DirectoryEntry>;

    fn into_iter(self) -> Self::IntoIter {
        self.entries.into_iter()
    }
}
