use std::{
    io::{Read, Seek, SeekFrom},
    path::Path,
};

use crate::{
    ext4::{
        ADDR_SIZE, DirectoryEntry, Ext4Error, Result,
        block::BlockGroupDescriptor,
        directory::Directory,
        extent::{Extent, ExtentHeader, ExtentIndex},
        file::File,
        inode::Inode,
        superblock::Superblock,
        xattr::{self, XAttrEntry},
    },
    utils::NormalizePath,
};

/// Represents an ext4 filesystem volume
pub struct Volume<R: Read + Seek> {
    reader: R,
    superblock: Superblock,
    block_size: u32,
}

impl<R: Read + Seek> Volume<R> {
    pub const MIN_BLOCK_SIZE: u32 = 1024;

    /// Create a new Volume from a reader
    pub fn new(mut reader: R) -> Result<Self> {
        reader.seek(SeekFrom::Start(Superblock::SUPERBLOCK_OFFSET))?;

        let mut sb_buf = vec![0u8; Superblock::SIZE];
        reader.read_exact(&mut sb_buf)?;

        let superblock = Superblock::parse(&sb_buf)?;
        let block_size = superblock.block_size();

        Ok(Self {
            reader,
            superblock,
            block_size,
        })
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
    pub fn read_block_group_descriptor(&mut self, bg_index: u32) -> Result<BlockGroupDescriptor> {
        if bg_index >= self.superblock.block_group_count() {
            return Err(Ext4Error::InvalidBlockGroup(bg_index));
        }

        let desc_size = self.superblock.descriptor_size() as u64;
        let first_block = if self.block_size == Self::MIN_BLOCK_SIZE {
            2
        } else {
            1
        };
        let offset = first_block * self.block_size as u64 + bg_index as u64 * desc_size;

        self.reader.seek(SeekFrom::Start(offset))?;

        let mut buffer = vec![0u8; desc_size as usize];
        self.reader.read_exact(&mut buffer)?;

        BlockGroupDescriptor::parse(&buffer)
    }

    /// Read a block from the filesystem
    pub fn read_block(&mut self, block_num: u64) -> Result<Vec<u8>> {
        let offset = block_num * self.block_size as u64;
        self.reader.seek(SeekFrom::Start(offset))?;

        let mut buffer = vec![0u8; self.block_size as usize];
        self.reader.read_exact(&mut buffer)?;

        Ok(buffer)
    }

    /// Read an inode from the filesystem
    pub fn read_inode(&mut self, inode_num: u32) -> Result<Inode> {
        if inode_num == 0 {
            return Err(Ext4Error::InvalidInode(inode_num));
        }

        let inodes_per_group = self.superblock.inodes_per_group;
        let inode_size = self.superblock.inode_size as u64;

        let bg_index = (inode_num - 1) / inodes_per_group;
        let inode_index = (inode_num - 1) % inodes_per_group;

        let inode_table_block = self
            .read_block_group_descriptor(bg_index)
            .map(|bg_desc| bg_desc.inode_table_first_block())?;

        let offset = inode_table_block * self.block_size as u64 + inode_index as u64 * inode_size;

        self.reader.seek(SeekFrom::Start(offset))?;

        let mut buffer = vec![0u8; inode_size as usize];
        self.reader.read_exact(&mut buffer)?;

        Inode::parse(&buffer)
    }

    /// Lookup a path and return its inode
    pub fn lookup_path(&mut self, path: impl AsRef<Path>) -> Result<Inode> {
        let path = path.as_ref().normalize()?;

        // Get components iterator
        let mut components = path
            .components()
            .filter(|c| matches!(c, std::path::Component::Normal(_)))
            .peekable();

        // If no components left, return root inode
        if components.peek().is_none() {
            return self.read_inode(Inode::ROOT_INODE);
        }

        let mut current_inode = self.read_inode(Inode::ROOT_INODE)?;

        for component in components {
            let directory = Directory::new(self, current_inode)?;
            let component_str = component
                .as_os_str()
                .to_str()
                .ok_or_else(|| Ext4Error::FileNotFound("Invalid UTF-8 in path".to_string()))?;

            current_inode = match directory.find(component_str) {
                Some(&entry) => self.read_inode(entry.inode)?,
                None => return Err(Ext4Error::FileNotFound(component_str.to_string())),
            };
        }

        Ok(current_inode)
    }

    /// Open a file for reading
    pub fn open_file<'a>(&'a mut self, path: impl AsRef<Path>) -> Result<File<'a, R>> {
        let inode = self.lookup_path(path)?;
        File::new(self, inode)
    }

    /// Open a directory for listing
    pub fn open_dir<'a>(&'a mut self, path: impl AsRef<Path>) -> Result<Directory<'a, R>> {
        let inode = self.lookup_path(path)?;
        Directory::new(self, inode)
    }

    /// Read all data from an inode (public API)
    pub fn read_inode_data_all(&mut self, inode: &Inode) -> Result<Vec<u8>> {
        let file_size = self.superblock.inode_size_file(inode);
        self.read_inode_data(inode, 0, file_size as usize)
    }

    /// Read extended attributes for an inode
    pub fn read_xattrs(&mut self, inode: &Inode) -> Result<Vec<XAttrEntry>> {
        let mut xattrs = Vec::new();

        // Add inline xattrs (already parsed during Inode::parse)
        xattrs.extend(inode.inline_xattrs.clone());

        // Check for external xattr block
        if inode.file_acl != 0 {
            let xattr_block = inode.xattr_block_number();

            let block_data = self.read_block(xattr_block)?;
            if let Ok(block_xattrs) = xattr::parse_xattrs_from_block(&block_data) {
                xattrs.extend(block_xattrs);
            }
        }

        Ok(xattrs)
    }

    /// Read inode data at a given offset
    pub(crate) fn read_inode_data(
        &mut self,
        inode: &Inode,
        offset: u64,
        length: usize,
    ) -> Result<Vec<u8>> {
        let file_size = self.superblock.inode_size_file(inode);

        if offset >= file_size {
            return Err(Ext4Error::ReadBeyondEof);
        }

        let actual_length = std::cmp::min(length, (file_size - offset) as usize);
        let mut result = vec![0u8; actual_length];

        if inode.uses_extents() {
            self.read_via_extents(inode, offset, &mut result)?;
        } else {
            self.read_via_indirect(inode, offset, &mut result)?;
        }

        Ok(result)
    }

    fn read_via_extents(&mut self, inode: &Inode, offset: u64, buf: &mut [u8]) -> Result<()> {
        let extents = self.parse_extent_tree(&inode.block)?;
        let mut bytes_read = 0;

        for extent in extents {
            let extent_start = extent.first_block as u64 * self.block_size as u64;
            let extent_len = extent.get_actual_len() as u64 * self.block_size as u64;
            let extent_end = extent_start + extent_len;

            if offset + buf.len() as u64 <= extent_start || offset >= extent_end {
                continue;
            }

            let read_start = offset.saturating_sub(extent_start);
            let read_end = std::cmp::min(extent_len, offset + buf.len() as u64 - extent_start);
            let to_read = (read_end - read_start) as usize;

            let physical_offset = extent.start_block() * self.block_size as u64 + read_start;
            self.reader.seek(SeekFrom::Start(physical_offset))?;
            self.reader
                .read_exact(&mut buf[bytes_read..bytes_read + to_read])?;

            bytes_read += to_read;
            if bytes_read >= buf.len() {
                break;
            }
        }

        Ok(())
    }

    fn read_via_indirect(&mut self, inode: &Inode, offset: u64, buf: &mut [u8]) -> Result<()> {
        let block_size = self.block_size as u64;
        let start_block = offset / block_size;
        let end_block = (offset + buf.len() as u64).div_ceil(block_size);
        let mut bytes_read = 0;

        for block_idx in start_block..end_block {
            let block_offset = if block_idx == start_block {
                (offset % block_size) as usize
            } else {
                0
            };

            let to_read = std::cmp::min(
                self.block_size as usize - block_offset,
                buf.len() - bytes_read,
            );

            let physical_block = self.resolve_block(inode, block_idx as u32)?;

            if physical_block == 0 {
                buf[bytes_read..bytes_read + to_read].fill(0);
            } else {
                let physical_offset = physical_block * block_size + block_offset as u64;
                self.reader.seek(SeekFrom::Start(physical_offset))?;
                self.reader
                    .read_exact(&mut buf[bytes_read..bytes_read + to_read])?;
            }

            bytes_read += to_read;
        }

        Ok(())
    }

    fn read_block_addr(&mut self, block_num: u64, index: u32) -> Result<u64> {
        if block_num == 0 {
            return Ok(0);
        }
        let block_data = self.read_block(block_num)?;
        let offset = (index * ADDR_SIZE) as usize;
        Ok(u32::from_le_bytes(block_data[offset..offset + 4].try_into().unwrap()) as u64)
    }

    /// Get block address from inode (handles indirect blocks)
    fn resolve_block(&mut self, inode: &Inode, logical_block: u32) -> Result<u64> {
        let addr_per_block = self.block_size / 4;

        if logical_block < Inode::DIRECT_BLOCKS {
            return Ok(inode.block[logical_block as usize] as u64);
        }

        let mut block_idx = logical_block - Inode::DIRECT_BLOCKS;

        if block_idx < addr_per_block {
            return self.read_block_addr(inode.block[Inode::INDIRECT_BLOCK_IDX] as u64, block_idx);
        }

        block_idx -= addr_per_block;

        if block_idx < addr_per_block * addr_per_block {
            let indirect = self.read_block_addr(
                inode.block[Inode::DOUBLE_INDIRECT_BLOCK_IDX] as u64,
                block_idx / addr_per_block,
            )?;
            return self.read_block_addr(indirect, block_idx % addr_per_block);
        }

        block_idx -= addr_per_block * addr_per_block;

        let double = self.read_block_addr(
            inode.block[Inode::TRIPLE_INDIRECT_BLOCK_IDX] as u64,
            block_idx / (addr_per_block * addr_per_block),
        )?;
        let indirect =
            self.read_block_addr(double, (block_idx / addr_per_block) % addr_per_block)?;
        self.read_block_addr(indirect, block_idx % addr_per_block)
    }

    fn parse_extent_tree(&mut self, block_data: &[u32; 15]) -> Result<Vec<Extent>> {
        let bytes: Vec<u8> = block_data
            .iter()
            .flat_map(|&word| word.to_le_bytes())
            .collect();

        self.parse_extent_tree_from_block(&bytes)
    }

    fn parse_extent_tree_from_block(&mut self, block_data: &[u8]) -> Result<Vec<Extent>> {
        let header = ExtentHeader::parse(&block_data[..ExtentHeader::SIZE])?;
        let mut extents = Vec::new();
        let mut offset = ExtentHeader::SIZE;

        for _ in 0..header.entries_count {
            if header.depth == 0 {
                extents.push(Extent::parse(&block_data[offset..offset + Extent::SIZE])?);
            } else {
                let index = ExtentIndex::parse(&block_data[offset..offset + ExtentIndex::SIZE])?;
                let child_block_data = self.read_block(index.leaf_block())?;
                extents.extend(self.parse_extent_tree_from_block(&child_block_data)?);
            }
            offset += Extent::SIZE;
        }

        Ok(extents)
    }

    /// Read all directory entries from an inode
    pub(crate) fn read_directory_entries(&mut self, inode: &Inode) -> Result<Vec<DirectoryEntry>> {
        if !inode.is_directory() {
            return Err(Ext4Error::NotADirectory);
        }

        let file_size = self.superblock.inode_size_file(inode);
        let data = self.read_inode_data(inode, 0, file_size as usize)?;

        let mut entries = Vec::new();
        let mut offset = 0;

        while offset < data.len() {
            if offset + DirectoryEntry::HEADER_SIZE > data.len() {
                break;
            }

            let inode_num = u32::from_le_bytes(
                data[offset..offset + 4]
                    .try_into()
                    .map_err(|_| Ext4Error::ReadBeyondEof)?,
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

    /// Read a symbolic link target
    pub fn read_symlink(&mut self, inode: &Inode) -> Result<String> {
        if !inode.is_symlink() {
            return Err(Ext4Error::InvalidPath("Not a symlink".to_string()));
        }

        let file_size = self.superblock.inode_size_file(inode);

        if file_size < Inode::FAST_SYMLINK_MAX_SIZE {
            // Fast symlink - stored in inode
            let link_data: Vec<u8> = inode
                .block
                .iter()
                .flat_map(|&word| word.to_le_bytes())
                .take(file_size as usize)
                .collect();
            Ok(String::from_utf8_lossy(&link_data).to_string())
        } else {
            // Slow symlink - stored in blocks
            let data = self.read_inode_data(inode, 0, file_size as usize)?;
            Ok(String::from_utf8_lossy(&data).to_string())
        }
    }
}
