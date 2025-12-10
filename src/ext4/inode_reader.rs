use std::io::{Read, Seek, SeekFrom};

use crate::{
    Volume,
    ext4::{
        ADDR_SIZE, Ext4Error, Result,
        extent::{Extent, ExtentHeader, ExtentIndex},
        inode::Inode,
    },
};

/// Low-level reader for inode data
pub(crate) struct InodeReader<R: Read + Seek> {
    reader: R,
    block_size: u32,
    inode: Inode,
}

impl<R: Read + Seek> InodeReader<R> {
    pub fn new<F: Fn() -> R>(volume: &Volume<R, F>, inode: Inode) -> Self {
        Self {
            reader: volume.reader(),
            block_size: volume.block_size(),
            inode,
        }
    }

    pub fn inode(&self) -> &Inode {
        &self.inode
    }

    pub fn size(&self) -> u64 {
        self.inode.size()
    }

    /// Read all data from the inode
    pub fn read_all(&mut self) -> Result<Vec<u8>> {
        self.read_data(0, self.size() as usize)
    }

    /// Read data at a given offset
    pub fn read_data(&mut self, offset: u64, length: usize) -> Result<Vec<u8>> {
        let file_size = self.inode.size();

        if offset >= file_size {
            return Err(Ext4Error::ReadBeyondEof);
        }

        let actual_length = std::cmp::min(length, (file_size - offset) as usize);
        let mut result = vec![0u8; actual_length];

        if self.inode.is_fast_symlink() {
            self.read_fast_symlink(offset, &mut result);
        } else if self.inode.uses_extents() {
            self.read_via_extents(offset, &mut result)?;
        } else {
            self.read_via_indirect(offset, &mut result)?;
        }

        Ok(result)
    }

    /// Read data from a fast symlink (inline in inode.block)
    fn read_fast_symlink(&self, offset: u64, buf: &mut [u8]) {
        let inline_data: Vec<u8> = self
            .inode
            .block
            .iter()
            .flat_map(|&word| word.to_le_bytes())
            .collect();

        let start = offset as usize;
        let end = start + buf.len();
        buf.copy_from_slice(&inline_data[start..end]);
    }

    fn read_via_extents(&mut self, offset: u64, buf: &mut [u8]) -> Result<()> {
        let extents = self.parse_extent_tree(&self.inode.block.clone())?;
        let mut bytes_read = 0;

        for extent in extents {
            let extent_start = extent.first_block() * self.block_size as u64;
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

    fn read_via_indirect(&mut self, offset: u64, buf: &mut [u8]) -> Result<()> {
        let block_size = self.block_size as u64;
        let start_block = offset / block_size;
        let end_block = (offset + buf.len() as u64).div_ceil(block_size);
        let mut bytes_read = 0;
        let inode_block = self.inode.block.clone();

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

            let physical_block = self.resolve_block(&inode_block, block_idx as u32)?;

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

    fn read_block(&mut self, block_num: u64) -> Result<Vec<u8>> {
        let offset = block_num * self.block_size as u64;
        self.reader.seek(SeekFrom::Start(offset))?;

        let mut buffer = vec![0u8; self.block_size as usize];
        self.reader.read_exact(&mut buffer)?;

        Ok(buffer)
    }

    fn read_block_addr(&mut self, block_num: u64, index: u32) -> Result<u64> {
        if block_num == 0 {
            return Ok(0);
        }
        let block_data = self.read_block(block_num)?;
        let offset = (index * ADDR_SIZE) as usize;
        Ok(u32::from_le_bytes(block_data[offset..offset + 4].try_into().unwrap()) as u64)
    }

    fn resolve_block(&mut self, inode_block: &[u32; 15], logical_block: u32) -> Result<u64> {
        let addr_per_block = self.block_size / 4;

        if logical_block < Inode::DIRECT_BLOCKS {
            return Ok(inode_block[logical_block as usize] as u64);
        }

        let mut block_idx = logical_block - Inode::DIRECT_BLOCKS;

        if block_idx < addr_per_block {
            return self.read_block_addr(inode_block[Inode::INDIRECT_BLOCK_IDX] as u64, block_idx);
        }

        block_idx -= addr_per_block;

        if block_idx < addr_per_block * addr_per_block {
            let indirect = self.read_block_addr(
                inode_block[Inode::DOUBLE_INDIRECT_BLOCK_IDX] as u64,
                block_idx / addr_per_block,
            )?;
            return self.read_block_addr(indirect, block_idx % addr_per_block);
        }

        block_idx -= addr_per_block * addr_per_block;

        let double = self.read_block_addr(
            inode_block[Inode::TRIPLE_INDIRECT_BLOCK_IDX] as u64,
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

        for _ in 0..header.entries_count() {
            if header.depth() == 0 {
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
}
