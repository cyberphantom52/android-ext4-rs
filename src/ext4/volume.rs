use std::{
    io::{Read, Seek, SeekFrom},
    path::Path,
};

use crate::{
    ext4::{
        DirectoryEntry, Ext4Error, Result,
        block::BlockGroupDescriptor,
        directory::Directory,
        extent::{Extent, ExtentHeader, ExtentIndex},
        file::File,
        inode::Inode,
        superblock::Superblock,
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
    /// Create a new Volume from a reader
    pub fn new(mut reader: R) -> Result<Self> {
        reader.seek(SeekFrom::Start(Superblock::SUPERBLOCK_OFFSET))?;

        let mut sb_buf = vec![0u8; 1024];
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
        let first_block = if self.block_size == 1024 { 2 } else { 1 };
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
        let mut bytes_read = 0;

        if inode.uses_extents() {
            let extents = self.parse_extent_tree(&inode.block)?;

            for extent in extents {
                let extent_start = extent.first_block as u64 * self.block_size as u64;
                let extent_end =
                    extent_start + extent.get_actual_len() as u64 * self.block_size as u64;

                if offset < extent_end && offset + actual_length as u64 > extent_start {
                    let read_start = offset.saturating_sub(extent_start);

                    let read_end = std::cmp::min(
                        extent_end - extent_start,
                        offset + actual_length as u64 - extent_start,
                    );

                    let physical_block = extent.start_block();
                    let physical_offset = physical_block * self.block_size as u64 + read_start;

                    self.reader.seek(SeekFrom::Start(physical_offset))?;

                    let to_read = (read_end - read_start) as usize;
                    self.reader
                        .read_exact(&mut result[bytes_read..bytes_read + to_read])?;
                    bytes_read += to_read;

                    if bytes_read >= actual_length {
                        break;
                    }
                }
            }
        } else {
            let start_block = (offset / self.block_size as u64) as usize;
            let end_block =
                ((offset + actual_length as u64).div_ceil(self.block_size as u64)) as usize;

            for block_idx in start_block..end_block {
                let physical_block = self.get_block_from_inode(inode, block_idx as u32)?;

                if physical_block == 0 {
                    let block_offset = if block_idx == start_block {
                        (offset % self.block_size as u64) as usize
                    } else {
                        0
                    };

                    let to_read = std::cmp::min(
                        self.block_size as usize - block_offset,
                        actual_length - bytes_read,
                    );

                    result[bytes_read..bytes_read + to_read].fill(0);
                    bytes_read += to_read;
                } else {
                    let block_offset = if block_idx == start_block {
                        (offset % self.block_size as u64) as usize
                    } else {
                        0
                    };

                    let physical_offset =
                        physical_block * self.block_size as u64 + block_offset as u64;
                    self.reader.seek(SeekFrom::Start(physical_offset))?;

                    let to_read = std::cmp::min(
                        self.block_size as usize - block_offset,
                        actual_length - bytes_read,
                    );

                    self.reader
                        .read_exact(&mut result[bytes_read..bytes_read + to_read])?;
                    bytes_read += to_read;
                }

                if bytes_read >= actual_length {
                    break;
                }
            }
        }

        Ok(result)
    }

    /// Get block address from inode (handles indirect blocks)
    fn get_block_from_inode(&mut self, inode: &Inode, logical_block: u32) -> Result<u64> {
        let addr_per_block = self.block_size / 4;

        // Direct blocks
        if logical_block < 12 {
            return Ok(inode.block[logical_block as usize] as u64);
        }

        let mut block_idx = logical_block - 12;

        // Single indirect
        if block_idx < addr_per_block {
            let indirect_block = inode.block[12] as u64;
            if indirect_block == 0 {
                return Ok(0);
            }
            let block_data = self.read_block(indirect_block)?;
            let offset = (block_idx * 4) as usize;
            let addr = u32::from_le_bytes([
                block_data[offset],
                block_data[offset + 1],
                block_data[offset + 2],
                block_data[offset + 3],
            ]);
            return Ok(addr as u64);
        }

        block_idx -= addr_per_block;

        // Double indirect
        if block_idx < addr_per_block * addr_per_block {
            let double_indirect = inode.block[13] as u64;
            if double_indirect == 0 {
                return Ok(0);
            }

            let first_level_idx = block_idx / addr_per_block;
            let second_level_idx = block_idx % addr_per_block;

            let first_level_data = self.read_block(double_indirect)?;
            let offset = (first_level_idx * 4) as usize;
            let indirect_block = u32::from_le_bytes([
                first_level_data[offset],
                first_level_data[offset + 1],
                first_level_data[offset + 2],
                first_level_data[offset + 3],
            ]) as u64;

            if indirect_block == 0 {
                return Ok(0);
            }

            let second_level_data = self.read_block(indirect_block)?;
            let offset = (second_level_idx * 4) as usize;
            let addr = u32::from_le_bytes([
                second_level_data[offset],
                second_level_data[offset + 1],
                second_level_data[offset + 2],
                second_level_data[offset + 3],
            ]);
            return Ok(addr as u64);
        }

        block_idx -= addr_per_block * addr_per_block;

        // Triple indirect
        let triple_indirect = inode.block[14] as u64;
        if triple_indirect == 0 {
            return Ok(0);
        }

        let first_level_idx = block_idx / (addr_per_block * addr_per_block);
        let second_level_idx = (block_idx / addr_per_block) % addr_per_block;
        let third_level_idx = block_idx % addr_per_block;

        let first_level_data = self.read_block(triple_indirect)?;
        let offset = (first_level_idx * 4) as usize;
        let double_indirect = u32::from_le_bytes([
            first_level_data[offset],
            first_level_data[offset + 1],
            first_level_data[offset + 2],
            first_level_data[offset + 3],
        ]) as u64;

        if double_indirect == 0 {
            return Ok(0);
        }

        let second_level_data = self.read_block(double_indirect)?;
        let offset = (second_level_idx * 4) as usize;
        let indirect_block = u32::from_le_bytes([
            second_level_data[offset],
            second_level_data[offset + 1],
            second_level_data[offset + 2],
            second_level_data[offset + 3],
        ]) as u64;

        if indirect_block == 0 {
            return Ok(0);
        }

        let third_level_data = self.read_block(indirect_block)?;
        let offset = (third_level_idx * 4) as usize;
        let addr = u32::from_le_bytes([
            third_level_data[offset],
            third_level_data[offset + 1],
            third_level_data[offset + 2],
            third_level_data[offset + 3],
        ]);

        Ok(addr as u64)
    }

    fn parse_extent_tree(&mut self, block_data: &[u32; 15]) -> Result<Vec<Extent>> {
        let bytes: Vec<u8> = block_data
            .iter()
            .flat_map(|&word| word.to_le_bytes())
            .collect();

        self.parse_extent_tree_from_block(&bytes)
    }

    fn parse_extent_tree_from_block(&mut self, block_data: &[u8]) -> Result<Vec<Extent>> {
        let header = ExtentHeader::parse(&block_data[..12])?;
        let mut extents = Vec::new();
        let mut offset = 12;

        for _ in 0..header.entries_count {
            if header.depth == 0 {
                extents.push(Extent::parse(&block_data[offset..offset + 12])?);
            } else {
                let index = ExtentIndex::parse(&block_data[offset..offset + 12])?;
                let child_block_data = self.read_block(index.leaf_block())?;
                extents.extend(self.parse_extent_tree_from_block(&child_block_data)?);
            }
            offset += 12;
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
            if offset + 8 > data.len() {
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

                let mut name = [0u8; 255];
                let actual_name_len = std::cmp::min(name_len as usize, 255);
                if offset + 8 + actual_name_len <= data.len() {
                    name[..actual_name_len]
                        .copy_from_slice(&data[offset + 8..offset + 8 + actual_name_len]);
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

        if file_size < 60 {
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
