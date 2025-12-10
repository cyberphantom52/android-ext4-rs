use std::path::{Component, Path, PathBuf};

use thiserror::Error;

/// The kind of structure being parsed when a nom error occurred
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParseContext {
    Superblock,
    BlockGroupDescriptor,
    Inode,
    ExtentHeader,
    ExtentIndex,
    Extent,
    XAttrHeader,
    XAttrIbodyHeader,
    XAttrEntry,
    Capability,
}

impl std::fmt::Display for ParseContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ParseContext::Superblock => write!(f, "superblock"),
            ParseContext::BlockGroupDescriptor => write!(f, "block group descriptor"),
            ParseContext::Inode => write!(f, "inode"),
            ParseContext::ExtentHeader => write!(f, "extent header"),
            ParseContext::ExtentIndex => write!(f, "extent index"),
            ParseContext::Extent => write!(f, "extent"),
            ParseContext::XAttrHeader => write!(f, "xattr header"),
            ParseContext::XAttrIbodyHeader => write!(f, "xattr ibody header"),
            ParseContext::XAttrEntry => write!(f, "xattr entry"),
            ParseContext::Capability => write!(f, "capability"),
        }
    }
}

#[derive(Error, Debug)]
pub enum Error {
    /// IO error during read/write operations
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// Nom parsing error with context about what was being parsed
    #[error("Failed to parse {context}: {kind:?}")]
    NomParse {
        context: ParseContext,
        kind: nom::error::ErrorKind,
    },

    /// Data validation error during parsing (e.g., buffer too small, invalid offsets)
    #[error("Invalid {context} data: {message}")]
    InvalidData {
        context: ParseContext,
        message: String,
    },

    /// Invalid inode number (e.g., inode 0 which is reserved)
    #[error("Invalid inode number {inode}: {reason}")]
    InvalidInode { inode: u32, reason: &'static str },

    /// Block group index out of range
    #[error("Block group {index} is out of range (filesystem has {count} block groups)")]
    InvalidBlockGroup { index: u32, count: u32 },

    /// File or directory not found during path lookup
    #[error("Path not found: '{path}' (component '{component}' does not exist)")]
    PathNotFound { path: String, component: String },

    /// Invalid UTF-8 in path component
    #[error("Invalid UTF-8 in path component")]
    InvalidUtf8InPath,

    /// Expected a directory but found something else
    #[error("Not a directory: '{0}'")]
    NotADirectory(String),

    /// Expected a regular file or symlink
    #[error("Not a regular file or symlink: '{0}'")]
    NotAFile(String),

    /// Path normalization failed (e.g., too many parent directory references)
    #[error("Invalid path '{path}': {reason}")]
    InvalidPath { path: String, reason: &'static str },

    /// Attempted to read beyond the end of file
    #[error("Read beyond end of file (file size: {file_size}, requested offset: {offset})")]
    ReadBeyondEof { file_size: u64, offset: u64 },

    /// Corrupted directory entry data
    #[error("Corrupted directory entry at offset {0}")]
    CorruptedDirectoryEntry(usize),

    /// XAttr name is out of bounds
    #[error("XAttr entry name out of bounds (name_len: {name_len}, available: {available})")]
    XAttrNameOutOfBounds { name_len: u8, available: usize },
}

impl Error {
    /// Create a NomParse error from a nom error and context
    pub fn nom_parse<I>(context: ParseContext, err: nom::error::Error<I>) -> Self {
        Error::NomParse {
            context,
            kind: err.code,
        }
    }

    /// Create an InvalidData error
    pub fn invalid_data(context: ParseContext, message: impl Into<String>) -> Self {
        Error::InvalidData {
            context,
            message: message.into(),
        }
    }

    /// Create an InvalidInode error for inode 0
    pub fn inode_zero() -> Self {
        Error::InvalidInode {
            inode: 0,
            reason: "inode 0 is reserved and cannot be read",
        }
    }
}

pub type Result<T> = std::result::Result<T, Error>;

pub trait NormalizePath {
    fn normalize(&self) -> Result<PathBuf>;
}

impl NormalizePath for Path {
    // Taken from: https://github.com/rust-lang/rust/pull/134696
    fn normalize(&self) -> Result<PathBuf> {
        let mut lexical = PathBuf::new();
        let mut iter = self.components().peekable();

        let root = match iter.peek() {
            Some(Component::ParentDir) => {
                return Err(Error::InvalidPath {
                    path: format!("{}", self.display()),
                    reason: "path cannot start with parent directory reference",
                });
            }
            Some(p @ Component::RootDir) | Some(p @ Component::CurDir) => {
                lexical.push(p);
                iter.next();
                lexical.as_os_str().len()
            }
            Some(Component::Prefix(prefix)) => {
                lexical.push(prefix.as_os_str());
                iter.next();
                if let Some(p @ Component::RootDir) = iter.peek() {
                    lexical.push(p);
                    iter.next();
                }
                lexical.as_os_str().len()
            }
            None => return Ok(PathBuf::new()),
            Some(Component::Normal(_)) => 0,
        };

        for component in iter {
            match component {
                Component::RootDir => unreachable!(),
                Component::Prefix(_) => {
                    return Err(Error::InvalidPath {
                        path: format!("{}", self.display()),
                        reason: "unexpected prefix in middle of path",
                    });
                }
                Component::CurDir => continue,
                Component::ParentDir => {
                    // It's an error if ParentDir causes us to go above the "root".
                    if lexical.as_os_str().len() == root {
                        return Err(Error::InvalidPath {
                            path: format!("{}", self.display()),
                            reason: "parent directory reference goes above root",
                        });
                    } else {
                        lexical.pop();
                    }
                }
                Component::Normal(path) => lexical.push(path),
            }
        }
        Ok(lexical)
    }
}
