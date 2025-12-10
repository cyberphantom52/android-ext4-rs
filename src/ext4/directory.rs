use std::io::{Read, Seek};
use std::path::{Path, PathBuf};

use crate::{
    DirectoryWalker, Error, Result, Volume,
    ext4::{DirectoryEntry, InodeReader, inode::Inode},
};

/// Represents a directory in the ext4 filesystem
pub struct Directory<R: Read + Seek, F: Fn() -> R> {
    pub(crate) volume: Volume<R, F>,
    pub(crate) path: PathBuf,
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

        let mut reader = InodeReader::new(volume, inode.clone());
        let data = reader.read_all()?;
        let entries = Volume::<R, F>::parse_directory_entries(&data)?;

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

    /// Get all entries
    pub fn entries(&self) -> &[DirectoryEntry] {
        &self.entries
    }

    /// Create a walker for recursive directory traversal
    pub fn walk(self) -> DirectoryWalker<R, F> {
        DirectoryWalker::new(self)
    }

    pub fn find(&self, name: &str) -> Option<&DirectoryEntry> {
        self.entries.iter().find(|entry| entry.name_str() == name)
    }
}

impl<R: Read + Seek, F: Fn() -> R> IntoIterator for Directory<R, F> {
    type Item = DirectoryEntry;
    type IntoIter = std::vec::IntoIter<DirectoryEntry>;

    fn into_iter(self) -> Self::IntoIter {
        self.entries.into_iter()
    }
}
