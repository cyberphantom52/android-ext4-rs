use std::{
    io::{Read, Seek},
    path::{Path, PathBuf},
};

use crate::{DirectoryEntry, Ext4Error, Inode, Result, Volume, ext4::directory::Directory};

/// A walker for recursive directory traversal
pub struct DirectoryWalker<'a, R: Read + Seek> {
    volume: &'a mut Volume<R>,
    stack: Vec<WalkEntry>,
    current_path: PathBuf,
}

struct WalkEntry {
    path: PathBuf,
    entries: Vec<DirectoryEntry>,
}

/// An item returned by the directory walker
#[derive(Debug, Clone)]
pub struct WalkItem {
    pub path: PathBuf,
    pub entry: DirectoryEntry,
    pub inode: Inode,
}

impl WalkItem {
    /// Check if this item is a directory
    pub fn is_directory(&self) -> bool {
        self.inode.is_directory()
    }

    /// Check if this item is a regular file
    pub fn is_file(&self) -> bool {
        self.inode.is_regular_file()
    }

    /// Check if this item is a symbolic link
    pub fn is_symlink(&self) -> bool {
        self.inode.is_symlink()
    }

    /// Get the name of this entry
    pub fn name(&self) -> &str {
        self.entry.name_str()
    }
}

impl<'a, R: Read + Seek> DirectoryWalker<'a, R> {
    pub(crate) fn new(directory: Directory<'a, R>) -> Self {
        let entries = directory.entries().to_vec();
        let initial_path = PathBuf::from("/");

        Self {
            volume: directory.volume,
            stack: vec![WalkEntry {
                path: initial_path.clone(),
                entries,
            }],
            current_path: initial_path,
        }
    }

    /// Create a walker starting from a specific path
    pub fn from_path(volume: &'a mut Volume<R>, path: impl AsRef<Path>) -> Result<Self> {
        let inode = volume.lookup_path(&path)?;

        if !inode.is_directory() {
            return Err(Ext4Error::NotADirectory);
        }

        let entries = volume.read_directory_entries(&inode)?;

        Ok(Self {
            volume,
            stack: vec![WalkEntry {
                path: path.as_ref().to_path_buf(),
                entries,
            }],
            current_path: path.as_ref().to_path_buf(),
        })
    }

    /// Get the current path being walked
    pub fn current_path(&self) -> &Path {
        &self.current_path
    }
}

impl<'a, R: Read + Seek> Iterator for DirectoryWalker<'a, R> {
    type Item = Result<WalkItem>;

    fn next(&mut self) -> Option<Self::Item> {
        while let Some(current) = self.stack.last_mut() {
            // Check if we've exhausted this level
            if current.entries.is_empty() {
                self.stack.pop();
                if let Some(parent) = self.stack.last() {
                    self.current_path = parent.path.clone();
                }
                continue;
            }

            // Get the next entry
            let entry = current.entries.pop()?;
            let entry_name = entry.name_str();

            // Skip "." and ".."
            if entry_name == "." || entry_name == ".." {
                continue;
            }

            // Build the full path
            self.current_path = current.path.join(entry_name);

            // Read the inode for this entry
            let inode = match self.volume.read_inode(entry.inode) {
                Ok(inode) => inode,
                Err(e) => return Some(Err(e)),
            };

            // If it's a directory, add it to the stack for later traversal
            if inode.is_directory() {
                match self.volume.read_directory_entries(&inode) {
                    Ok(entries) => {
                        self.stack.push(WalkEntry {
                            path: self.current_path.clone(),
                            entries,
                        });
                    }
                    Err(e) => return Some(Err(e)),
                }
            }

            return Some(Ok(WalkItem {
                path: self.current_path.clone(),
                entry,
                inode,
            }));
        }

        None
    }
}
