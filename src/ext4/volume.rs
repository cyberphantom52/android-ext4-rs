use std::{
    io::{Read, Seek, SeekFrom},
    path::{Path, PathBuf},
    sync::Arc,
};

use crate::{
    Directory, Error, File, Result,
    ext4::{
        InodeReader,
        block::BlockGroupDescriptor,
        inode::Inode,
        superblock::Superblock,
        xattr::{self, XAttrEntry},
    },
    utils::NormalizePath,
};

/// Represents an ext4 filesystem volume
#[derive(Debug)]
pub struct Volume<R: Read + Seek, F: Fn() -> R> {
    reader_factory: Arc<F>,
    superblock: Superblock,
    block_size: u32,
}

impl<R: Read + Seek, F: Fn() -> R> Clone for Volume<R, F> {
    fn clone(&self) -> Self {
        Self {
            reader_factory: Arc::clone(&self.reader_factory),
            superblock: self.superblock.clone(),
            block_size: self.block_size,
        }
    }
}

impl<R: Read + Seek, F: Fn() -> R> Volume<R, F> {
    pub const MIN_BLOCK_SIZE: u32 = 1024;

    /// Create a new Volume from a reader factory
    pub fn new(reader_factory: F) -> Result<Self> {
        let mut reader = reader_factory();
        reader.seek(SeekFrom::Start(Superblock::SUPERBLOCK_OFFSET))?;

        let mut sb_buf = vec![0u8; Superblock::SIZE];
        reader.read_exact(&mut sb_buf)?;

        let superblock = Superblock::parse(&sb_buf)?;
        let block_size = superblock.block_size();

        Ok(Self {
            reader_factory: Arc::new(reader_factory),
            superblock,
            block_size,
        })
    }

    /// Create a new reader from the factory
    pub fn reader(&self) -> R {
        (self.reader_factory)()
    }

    /// Get the volume name
    pub fn name(&self) -> Option<String> {
        if self.superblock.volume_name().is_empty() {
            None
        } else {
            Some(self.superblock.volume_name().to_string())
        }
    }

    /// Get the superblock
    pub fn superblock(&self) -> &Superblock {
        &self.superblock
    }

    /// Get the block size
    pub fn block_size(&self) -> u32 {
        self.block_size
    }

    /// Read a block group descriptor
    pub fn read_block_group_descriptor(&self, bg_index: u32) -> Result<BlockGroupDescriptor> {
        let block_group_count = self.superblock.block_group_count();
        if bg_index >= block_group_count {
            return Err(Error::InvalidBlockGroup {
                index: bg_index,
                count: block_group_count,
            });
        }

        let desc_size = self.superblock.descriptor_size() as u64;
        let first_block = if self.block_size == Self::MIN_BLOCK_SIZE {
            2
        } else {
            1
        };
        let offset = first_block * self.block_size as u64 + bg_index as u64 * desc_size;

        let mut reader = self.reader();
        reader.seek(SeekFrom::Start(offset))?;

        let mut buffer = vec![0u8; desc_size as usize];
        reader.read_exact(&mut buffer)?;

        BlockGroupDescriptor::parse(&buffer)
    }

    /// Read a block from the filesystem
    pub fn read_block(&self, block_num: u64) -> Result<Vec<u8>> {
        let offset = block_num * self.block_size as u64;

        let mut reader = self.reader();
        reader.seek(SeekFrom::Start(offset))?;

        let mut buffer = vec![0u8; self.block_size as usize];
        reader.read_exact(&mut buffer)?;

        Ok(buffer)
    }

    /// Read an inode from the filesystem
    pub fn read_inode(&self, inode_num: u32) -> Result<Inode> {
        if inode_num == 0 {
            return Err(Error::inode_zero());
        }

        let inodes_per_group = self.superblock.inodes_per_group();
        let inode_size = self.superblock.inode_size();

        let bg_index = (inode_num - 1) / inodes_per_group;
        let inode_index = (inode_num - 1) % inodes_per_group;

        let inode_table_block = self
            .read_block_group_descriptor(bg_index)
            .map(|bg_desc| bg_desc.inode_table_first_block())?;

        let offset = inode_table_block * self.block_size as u64 + inode_index as u64 * inode_size;

        let mut reader = self.reader();
        reader.seek(SeekFrom::Start(offset))?;

        let mut buffer = vec![0u8; inode_size as usize];
        reader.read_exact(&mut buffer)?;

        Inode::parse(&buffer)
    }

    /// Lookup a path and return its inode along with the normalized path
    fn lookup_path_with_normalized(&self, path: impl AsRef<Path>) -> Result<(Inode, PathBuf)> {
        let original_path = path.as_ref();
        let normalized_path = original_path.normalize()?;

        // Get components iterator
        let mut components = normalized_path
            .components()
            .filter(|c| matches!(c, std::path::Component::Normal(_)))
            .peekable();

        // If no components left, return root inode
        if components.peek().is_none() {
            return Ok((self.read_inode(Inode::ROOT_INODE)?, PathBuf::from("/")));
        }

        let mut current_inode = self.read_inode(Inode::ROOT_INODE)?;
        let mut current_path = PathBuf::from("/");

        for component in components {
            let directory = Directory::new(self, current_inode, &current_path)?;
            let component_str = component
                .as_os_str()
                .to_str()
                .ok_or(Error::InvalidUtf8InPath)?;

            current_inode = match directory.find(component_str) {
                Some(entry) => self.read_inode(entry.inode)?,
                None => {
                    return Err(Error::PathNotFound {
                        path: format!("{}", original_path.display()),
                        component: component_str.to_string(),
                    });
                }
            };
            current_path.push(component_str);
        }

        Ok((current_inode, current_path))
    }

    /// Lookup a path and return its inode
    pub fn lookup_path(&self, path: impl AsRef<Path>) -> Result<Inode> {
        self.lookup_path_with_normalized(path)
            .map(|(inode, _)| inode)
    }

    /// Open a file for reading
    pub fn open_file(&self, path: impl AsRef<Path>) -> Result<File<R>> {
        let (inode, normalized_path) = self.lookup_path_with_normalized(&path)?;
        File::new(self, inode, normalized_path)
    }

    /// Open a directory for listing
    pub fn open_dir(&self, path: impl AsRef<Path>) -> Result<Directory<R, F>> {
        let (inode, normalized_path) = self.lookup_path_with_normalized(&path)?;
        Directory::new(self, inode, normalized_path)
    }

    /// Read all data from an inode
    pub fn read_inode_data(&self, inode: &Inode) -> Result<Vec<u8>> {
        let mut reader = InodeReader::new(self);
        reader.read_all(inode)
    }

    /// Read extended attributes for an inode
    pub fn read_xattrs(&self, inode: &Inode) -> Result<Vec<XAttrEntry>> {
        let mut xattrs = Vec::new();

        // Add inline xattrs (already parsed during Inode::parse)
        xattrs.extend(inode.xattrs().iter().cloned());

        // Check for external xattr block
        if let Some(xattr_block) = inode.xattr_block_number() {
            let block_data = self.read_block(xattr_block)?;
            if let Ok(block_xattrs) = xattr::parse_xattrs_from_block(&block_data) {
                xattrs.extend(block_xattrs);
            }
        }

        Ok(xattrs)
    }
}
