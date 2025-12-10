use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

use crate::{Error, Result, Volume, ext4::InodeReader, ext4::inode::Inode};

/// Represents a file in the ext4 filesystem
pub struct File<R: Read + Seek> {
    reader: InodeReader<R>,
    inode: Inode,
    position: u64,
    path: PathBuf,
}

impl<R: Read + Seek> File<R> {
    /// Create a new File from a volume, inode, and path
    /// Accepts regular files and symlinks
    pub(crate) fn new<F: Fn() -> R>(
        volume: &Volume<R, F>,
        inode: Inode,
        path: impl Into<PathBuf>,
    ) -> Result<Self> {
        let path = path.into();
        if !inode.is_regular_file() && !inode.is_symlink() {
            return Err(Error::NotAFile(format!("{}", path.display())));
        }

        Ok(Self {
            reader: InodeReader::new(volume),
            inode,
            position: 0,
            path,
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

    /// Check if this file is a symlink
    pub fn is_symlink(&self) -> bool {
        self.inode().is_symlink()
    }

    /// Get the path of this file
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Read all contents of the file
    pub fn read_all(&mut self) -> Result<Vec<u8>> {
        self.position = 0;
        self.reader.read_all(&self.inode)
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
            .read_data(&self.inode, self.position, to_read)
            .map_err(std::io::Error::other)?;

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
