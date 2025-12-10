use std::{
    io::{Read, Seek},
    path::{Path, PathBuf},
};

use crate::ext4::{
    DirectoryEntry, Result, Volume,
    directory::Directory,
    inode::{FileType, Inode, Mode},
};

/// A walker for recursive directory traversal
pub struct DirectoryWalker<R: Read + Seek, F: Fn() -> R> {
    volume: Volume<R, F>,
    stack: Vec<WalkEntry>,
}

struct WalkEntry {
    path: PathBuf,
    entries: Vec<DirectoryEntry>,
}

#[derive(Debug, Clone)]
pub struct EntryAttributes {
    mode: Mode,
    uid: u32,
    gid: u32,
    selinux: Option<String>,
    capabilities: Option<String>,
}

impl EntryAttributes {
    /// Get the mode of the entry
    pub fn mode(&self) -> Mode {
        self.mode
    }

    /// Get the user ID of the entry
    pub fn uid(&self) -> u32 {
        self.uid
    }

    /// Get the group ID of the entry
    pub fn gid(&self) -> u32 {
        self.gid
    }

    /// Get the SELinux context of the entry, if available
    pub fn selinux(&self) -> Option<&str> {
        self.selinux.as_deref()
    }

    /// Get the capabilities of the entry, if available
    pub fn capabilities(&self) -> Option<&str> {
        self.capabilities.as_deref()
    }

    pub fn mode_string(&self) -> String {
        self.mode.permissions_string()
    }

    pub fn mode_with_caps(&self) -> String {
        match &self.capabilities {
            Some(cap) => format!("{}{}", self.mode.permissions_string(), cap),
            None => self.mode_string(),
        }
    }
}

/// An item returned by the directory walker
#[derive(Debug, Clone)]
pub struct WalkItem {
    path: PathBuf,
    entry: DirectoryEntry,
    inode: Inode,
    attributes: EntryAttributes,
}

impl WalkItem {
    pub fn inode(&self) -> &Inode {
        &self.inode
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn attributes(&self) -> &EntryAttributes {
        &self.attributes
    }

    pub fn r#type(&self) -> FileType {
        self.inode
            .mode()
            .file_type()
            .expect("Invalid file type in inode")
    }

    /// Get the name of this entry
    pub fn name(&self) -> &str {
        self.entry.name_str()
    }
}

impl<R: Read + Seek, F: Fn() -> R> DirectoryWalker<R, F> {
    pub(crate) fn new(directory: Directory<R, F>) -> Self {
        let entries = directory.entries().unwrap_or_default();
        let path = directory.path().to_path_buf();

        Self {
            volume: directory.volume,
            stack: vec![WalkEntry { path, entries }],
        }
    }

    /// Create a walker starting from a specific path
    pub fn from_path(volume: &Volume<R, F>, path: impl AsRef<Path>) -> Result<Self> {
        let directory = volume.open_dir(&path)?;
        Ok(Self::new(directory))
    }

    pub fn current_path(&self) -> Option<&Path> {
        self.stack.last().map(|frame| frame.path.as_path())
    }
}

impl<R: Read + Seek, F: Fn() -> R> Iterator for DirectoryWalker<R, F> {
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
                let directory = Directory::new(&self.volume, inode.clone(), &item_path).ok()?;
                match directory.entries() {
                    Ok(entries) => {
                        self.stack.push(WalkEntry {
                            path: directory.path().to_path_buf(),
                            entries,
                        });
                    }
                    Err(e) => return Some(Err(e)),
                }
            }

            let xattrs = self.volume.read_xattrs(&inode).unwrap_or_default();
            let attributes = EntryAttributes {
                mode: inode.mode(),
                uid: inode.uid(),
                gid: inode.gid(),
                selinux: xattrs.iter().find_map(|x| x.selinux_context()),
                capabilities: xattrs.iter().find_map(|x| x.capability_string()),
            };

            return Some(Ok(WalkItem {
                path: item_path,
                entry,
                inode,
                attributes,
            }));
        }

        None
    }
}
