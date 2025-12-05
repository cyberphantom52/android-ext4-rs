use std::{
    io::{Read, Seek},
    path::{Path, PathBuf},
};

use crate::{DirectoryEntry, Inode, Result, Volume, ext4::directory::Directory};

/// A walker for recursive directory traversal
pub struct DirectoryWalker<'a, R: Read + Seek> {
    volume: &'a mut Volume<R>,
    stack: Vec<WalkEntry>,
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

        Self {
            volume: directory.volume,
            stack: vec![WalkEntry {
                path: PathBuf::from("/"),
                entries,
            }],
        }
    }

    /// Create a walker starting from a specific path
    pub fn from_path(volume: &'a mut Volume<R>, path: impl AsRef<Path>) -> Result<Self> {
        let inode = volume.lookup_path(&path)?;
        let directory = Directory::new(volume, inode)?;
        Ok(Self::new(directory))
    }

    pub fn current_path(&self) -> Option<&Path> {
        self.stack.last().map(|frame| frame.path.as_path())
    }
}

impl<'a, R: Read + Seek> Iterator for DirectoryWalker<'a, R> {
    type Item = Result<WalkItem>;

    fn next(&mut self) -> Option<Self::Item> {
        while let Some(current) = self.stack.last_mut() {
            let entry = match current.entries.pop() {
                Some(e) => e,
                None => {
                    self.stack.pop();
                    continue;
                }
            };

            let entry_name = entry.name_str();
            if entry_name == "." || entry_name == ".." {
                continue;
            }

            let item_path = current.path.join(entry_name);

            let inode = match self.volume.read_inode(entry.inode) {
                Ok(inode) => inode,
                Err(e) => return Some(Err(e)),
            };

            if inode.is_directory() {
                match self.volume.read_directory_entries(&inode) {
                    Ok(entries) => {
                        self.stack.push(WalkEntry {
                            path: item_path.clone(),
                            entries,
                        });
                    }
                    Err(e) => return Some(Err(e)),
                }
            }

            return Some(Ok(WalkItem {
                path: item_path,
                entry,
                inode,
            }));
        }

        None
    }
}
