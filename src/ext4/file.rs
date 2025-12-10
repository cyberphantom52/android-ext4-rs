use std::io::{Read, Seek, SeekFrom};

use crate::{Ext4Error, Result, Volume, ext4::InodeReader, ext4::inode::Inode};

/// Represents a file in the ext4 filesystem
pub struct File<R: Read + Seek> {
    reader: InodeReader<R>,
    position: u64,
}

impl<R: Read + Seek> File<R> {
    /// Create a new File from a volume and inode
    /// Accepts regular files and symlinks
    pub(crate) fn new<F: Fn() -> R>(volume: &Volume<R, F>, inode: Inode) -> Result<Self> {
        if !inode.is_regular_file() && !inode.is_symlink() {
            return Err(Ext4Error::InvalidPath(
                "Not a regular file or symlink".to_string(),
            ));
        }

        Ok(Self {
            reader: InodeReader::new(volume, inode),
            position: 0,
        })
    }

    /// Get the file size
    pub fn size(&self) -> u64 {
        self.reader.size()
    }

    /// Get the current position in the file
    pub fn position(&self) -> u64 {
        self.position
    }

    /// Get a reference to the inode
    pub fn inode(&self) -> &Inode {
        self.reader.inode()
    }

    /// Check if this file is a symlink
    pub fn is_symlink(&self) -> bool {
        self.reader.inode().is_symlink()
    }

    /// Read all contents of the file
    pub fn read_all(&mut self) -> Result<Vec<u8>> {
        self.position = 0;
        self.reader.read_all()
    }
}

impl<R: Read + Seek> Read for File<R> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        if self.position >= self.size() {
            return Ok(0); // EOF
        }

        let to_read = std::cmp::min(buf.len(), (self.size() - self.position) as usize);

        let data = self
            .reader
            .read_data(self.position, to_read)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;

        let bytes_read = data.len();
        buf[..bytes_read].copy_from_slice(&data);
        self.position += bytes_read as u64;

        Ok(bytes_read)
    }
}

impl<R: Read + Seek> Seek for File<R> {
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
