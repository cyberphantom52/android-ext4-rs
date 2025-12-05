use std::io::{Read, Seek};

use crate::{
    DirectoryWalker,
    ext4::{DirectoryEntry, Ext4Error, Inode, Result, Volume},
};

/// Represents a directory in the ext4 filesystem
pub struct Directory<'a, R: Read + Seek> {
    pub volume: &'a mut Volume<R>,
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

pub struct DirectoryIterator<'a, R: Read + Seek> {
    directory: Directory<'a, R>,
    index: usize,
}

impl<'a, R: Read + Seek> IntoIterator for Directory<'a, R> {
    type Item = DirectoryEntry;
    type IntoIter = DirectoryIterator<'a, R>;

    fn into_iter(self) -> Self::IntoIter {
        DirectoryIterator {
            directory: self,
            index: 0,
        }
    }
}

impl<'a, R: Read + Seek> Iterator for DirectoryIterator<'a, R> {
    type Item = DirectoryEntry;

    fn next(&mut self) -> Option<Self::Item> {
        match self.directory.entries().get(self.index) {
            Some(&entry) => {
                self.index += 1;
                Some(entry)
            }
            None => None,
        }
    }
}
