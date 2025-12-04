use std::io::{Read, Seek, SeekFrom};

use crate::ext4::{
    DirectoryEntry, Ext4Error, Result,
    block::BlockGroupDescriptor,
    extent::{Extent, ExtentHeader, ExtentIndex},
    inode::Inode,
    superblock::Superblock,
};

pub struct Ext4Reader<R: Read + Seek> {
    reader: R,
    superblock: Superblock,
    block_size: u32,
}

impl<R: Read + Seek> Ext4Reader<R> {
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

    pub fn superblock(&self) -> &Superblock {
        &self.superblock
    }

    pub fn block_size(&self) -> u32 {
        self.block_size
    }

    pub fn read_block(&mut self, block_num: u64) -> Result<Vec<u8>> {
        let offset = block_num * self.block_size as u64;
        self.reader.seek(SeekFrom::Start(offset))?;

        let mut buffer = vec![0u8; self.block_size as usize];
        self.reader.read_exact(&mut buffer)?;

        Ok(buffer)
    }

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

    pub fn read_inode(&mut self, inode_num: u32) -> Result<Inode> {
        if inode_num == 0 {
            return Err(Ext4Error::InvalidInode(inode_num));
        }

        let inodes_per_group = self.superblock.inodes_per_group;
        let inode_size = self.superblock.inode_size as u64;

        let bg_index = (inode_num - 1) / inodes_per_group;
        let inode_index = (inode_num - 1) % inodes_per_group;

        let bg_desc = self.read_block_group_descriptor(bg_index)?;
        let inode_table_block = bg_desc.inode_table_first_block();

        let offset = inode_table_block * self.block_size as u64 + inode_index as u64 * inode_size;

        self.reader.seek(SeekFrom::Start(offset))?;

        let mut buffer = vec![0u8; inode_size as usize];
        self.reader.read_exact(&mut buffer)?;

        let inode = Inode::parse(&buffer)?;

        Ok(inode)
    }

    pub fn read_inode_data(
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

    fn get_block_from_inode(&mut self, inode: &Inode, logical_block: u32) -> Result<u64> {
        let addr_per_block = self.block_size / 4;

        if logical_block < 12 {
            return Ok(inode.block[logical_block as usize] as u64);
        }

        let mut block_idx = logical_block - 12;

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
        let header_bytes: [u8; 12] = [
            (block_data[0] & 0xFF) as u8,
            ((block_data[0] >> 8) & 0xFF) as u8,
            ((block_data[0] >> 16) & 0xFF) as u8,
            ((block_data[0] >> 24) & 0xFF) as u8,
            (block_data[1] & 0xFF) as u8,
            ((block_data[1] >> 8) & 0xFF) as u8,
            ((block_data[1] >> 16) & 0xFF) as u8,
            ((block_data[1] >> 24) & 0xFF) as u8,
            (block_data[2] & 0xFF) as u8,
            ((block_data[2] >> 8) & 0xFF) as u8,
            ((block_data[2] >> 16) & 0xFF) as u8,
            ((block_data[2] >> 24) & 0xFF) as u8,
        ];

        let header = ExtentHeader::parse(&header_bytes)?;

        let mut extents = Vec::new();

        if header.depth == 0 {
            let mut offset = 12;
            for _ in 0..header.entries_count {
                let extent_bytes = self.u32_array_to_bytes(block_data, offset, 12);
                let extent = Extent::parse(&extent_bytes)?;
                extents.push(extent);
                offset += 12;
            }
        } else {
            let mut offset = 12;
            for _ in 0..header.entries_count {
                let index_bytes = self.u32_array_to_bytes(block_data, offset, 12);
                let index = ExtentIndex::parse(&index_bytes)?;

                let leaf_block = index.leaf_block();
                let block_data = self.read_block(leaf_block)?;

                let child_extents = self.parse_extent_tree_from_block(&block_data)?;
                extents.extend(child_extents);

                offset += 12;
            }
        }

        Ok(extents)
    }

    fn parse_extent_tree_from_block(&mut self, block_data: &[u8]) -> Result<Vec<Extent>> {
        let header = ExtentHeader::parse(&block_data[..12])?;

        let mut extents = Vec::new();

        if header.depth == 0 {
            let mut offset = 12;
            for _ in 0..header.entries_count {
                let extent = Extent::parse(&block_data[offset..offset + 12])?;
                extents.push(extent);
                offset += 12;
            }
        } else {
            let mut offset = 12;
            for _ in 0..header.entries_count {
                let index = ExtentIndex::parse(&block_data[offset..offset + 12])?;

                let leaf_block = index.leaf_block();
                let child_block_data = self.read_block(leaf_block)?;

                let child_extents = self.parse_extent_tree_from_block(&child_block_data)?;
                extents.extend(child_extents);

                offset += 12;
            }
        }

        Ok(extents)
    }

    fn u32_array_to_bytes(&self, data: &[u32; 15], offset: usize, length: usize) -> Vec<u8> {
        let mut result = Vec::with_capacity(length);
        let start_idx = offset / 4;
        let start_byte = offset % 4;

        for i in 0..length.div_ceil(4) {
            if start_idx + i < 15 {
                let val = data[start_idx + i];
                result.push((val & 0xFF) as u8);
                result.push(((val >> 8) & 0xFF) as u8);
                result.push(((val >> 16) & 0xFF) as u8);
                result.push(((val >> 24) & 0xFF) as u8);
            }
        }

        result[start_byte..start_byte + length].to_vec()
    }

    pub fn read_directory(&mut self, inode: &Inode) -> Result<Vec<DirectoryEntry>> {
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

            let inode_num = u32::from_le_bytes([
                data[offset],
                data[offset + 1],
                data[offset + 2],
                data[offset + 3],
            ]);

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

    pub fn find_entry_in_directory(
        &mut self,
        dir_inode: &Inode,
        name: &str,
    ) -> Result<Option<DirectoryEntry>> {
        let entries = self.read_directory(dir_inode)?;

        for entry in entries {
            if entry.name_str() == name {
                return Ok(Some(entry));
            }
        }

        Ok(None)
    }

    pub fn lookup_path(&mut self, path: &str) -> Result<Inode> {
        let path = path.trim_start_matches('/');

        if path.is_empty() {
            return self.read_inode(Inode::ROOT_INODE);
        }

        let components: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();

        let mut current_inode = self.read_inode(Inode::ROOT_INODE)?;

        for component in components {
            if !current_inode.is_directory() {
                return Err(Ext4Error::NotADirectory);
            }

            match self.find_entry_in_directory(&current_inode, component)? {
                Some(entry) => {
                    current_inode = self.read_inode(entry.inode)?;
                }
                None => {
                    return Err(Ext4Error::FileNotFound(component.to_string()));
                }
            }
        }

        Ok(current_inode)
    }

    pub fn read_file(&mut self, path: &str) -> Result<Vec<u8>> {
        let inode = self.lookup_path(path)?;

        if !inode.is_regular_file() {
            return Err(Ext4Error::InvalidPath(format!(
                "{} is not a regular file",
                path
            )));
        }

        let file_size = self.superblock.inode_size_file(&inode);
        self.read_inode_data(&inode, 0, file_size as usize)
    }

    pub fn list_directory(&mut self, path: &str) -> Result<Vec<DirectoryEntry>> {
        let inode = self.lookup_path(path)?;

        if !inode.is_directory() {
            return Err(Ext4Error::NotADirectory);
        }

        self.read_directory(&inode)
    }

    pub fn read_symlink(&mut self, inode: &Inode) -> Result<String> {
        if !inode.is_symlink() {
            return Err(Ext4Error::InvalidPath("Not a symlink".to_string()));
        }

        let file_size = self.superblock.inode_size_file(inode);

        if file_size < 60 {
            let link_data = unsafe {
                std::slice::from_raw_parts(inode.block.as_ptr() as *const u8, file_size as usize)
            };
            Ok(String::from_utf8_lossy(link_data).to_string())
        } else {
            let data = self.read_inode_data(inode, 0, file_size as usize)?;
            Ok(String::from_utf8_lossy(&data).to_string())
        }
    }

    pub fn file_size(&mut self, path: &str) -> Result<u64> {
        let inode = self.lookup_path(path)?;
        Ok(self.superblock.inode_size_file(&inode))
    }

    pub fn exists(&mut self, path: &str) -> bool {
        self.lookup_path(path).is_ok()
    }

    pub fn is_directory(&mut self, path: &str) -> Result<bool> {
        let inode = self.lookup_path(path)?;
        Ok(inode.is_directory())
    }

    pub fn is_file(&mut self, path: &str) -> Result<bool> {
        let inode = self.lookup_path(path)?;
        Ok(inode.is_regular_file())
    }

    pub fn is_symlink(&mut self, path: &str) -> Result<bool> {
        let inode = self.lookup_path(path)?;
        Ok(inode.is_symlink())
    }
}
