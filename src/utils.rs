use std::path::{Component, Path, PathBuf};

use crate::Ext4Error;

pub trait NormalizePath {
    fn normalize(&self) -> Result<PathBuf, Ext4Error>;
}

impl NormalizePath for Path {
    // Taken from: https://github.com/rust-lang/rust/pull/134696
    fn normalize(&self) -> Result<PathBuf, Ext4Error> {
        let mut lexical = PathBuf::new();
        let mut iter = self.components().peekable();

        let root = match iter.peek() {
            Some(Component::ParentDir) => {
                return Err(Ext4Error::InvalidPath(format!(
                    "Failed to Normalize: {}",
                    self.display()
                )));
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
                    return Err(Ext4Error::InvalidPath(format!(
                        "Failed to Normalize: {}",
                        self.display()
                    )));
                }
                Component::CurDir => continue,
                Component::ParentDir => {
                    // It's an error if ParentDir causes us to go above the "root".
                    if lexical.as_os_str().len() == root {
                        return Err(Ext4Error::InvalidPath(format!(
                            "Failed to Normalize: {}",
                            self.display()
                        )));
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
