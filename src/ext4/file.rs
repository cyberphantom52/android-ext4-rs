use std::io::{Read, Seek, SeekFrom};

use crate::ext4::{Ext4Error, Inode, Result, Volume};

/// Represents a file in the ext4 filesystem
pub struct File<'a, R: Read + Seek> {
    volume: &'a mut Volume<R>,
    inode: Inode,
    position: u64,
}

impl<'a, R: Read + Seek> File<'a, R> {
    /// Create a new File from a volume and inode
    pub(crate) fn new(volume: &'a mut Volume<R>, inode: Inode) -> Result<Self> {
        if !inode.is_regular_file() {
            return Err(Ext4Error::InvalidPath("Not a regular file".to_string()));
        }

        Ok(Self {
            volume,
            inode,
            position: 0,
        })
    }

    /// Get the file size
    pub fn size(&self) -> u64 {
        self.inode.size()
    }

    /// Get the current position in the file
    pub fn position(&self) -> u64 {
        self.position
    }

    /// Get a reference to the inode
    pub fn inode(&self) -> &Inode {
        &self.inode
    }

    /// Read all contents of the file
    pub fn read_all(&mut self) -> Result<Vec<u8>> {
        self.position = 0;
        self.volume
            .read_inode_data(&self.inode, 0, self.size() as usize)
    }
}

impl<'a, R: Read + Seek> Read for File<'a, R> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        if self.position >= self.size() {
            return Ok(0); // EOF
        }

        let to_read = std::cmp::min(buf.len(), (self.size() - self.position) as usize);

        let data = self
            .volume
            .read_inode_data(&self.inode, self.position, to_read)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;

        let bytes_read = data.len();
        buf[..bytes_read].copy_from_slice(&data);
        self.position += bytes_read as u64;

        Ok(bytes_read)
    }
}

impl<'a, R: Read + Seek> Seek for File<'a, R> {
    fn seek(&mut self, pos: SeekFrom) -> std::io::Result<u64> {
        let new_pos = match pos {
            SeekFrom::Start(offset) => offset as i64,
            SeekFrom::End(offset) => self.size() as i64 + offset,
            SeekFrom::Current(offset) => self.position as i64 + offset,
        };

        if new_pos < 0 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "Invalid seek to negative position",
            ));
        }

        self.position = new_pos as u64;
        Ok(self.position)
    }
}
