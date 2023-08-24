use std::{
    fs::{self, ReadDir},
    path::Path,
};

use crate::{Maildir, Result};

/// An iterator over the maildir subdirectories. This iterator produces a
/// `Result<Maildir>`, which can be an `Err` if an error was encountered while
/// trying to read file system properties on a particular entry. Only
/// subdirectories starting with a single period are included.
pub struct Folders {
    readdir: Option<ReadDir>,
}

impl Folders {
    pub(crate) fn new<P: AsRef<Path>>(path: P) -> Folders {
        Folders {
            readdir: fs::read_dir(path).ok(),
        }
    }
}

impl Iterator for Folders {
    type Item = Result<Maildir>;

    fn next(&mut self) -> Option<Result<Maildir>> {
        if let Some(ref mut readdir) = self.readdir {
            for entry in readdir {
                let path = match entry {
                    Err(e) => return Some(Err(e.into())),
                    Ok(e) => e.path(),
                };

                if !path.starts_with(".") || path.starts_with("..") || !path.is_dir() {
                    continue;
                }

                return Some(Ok(Maildir::from(path)));
            }
        }

        None
    }
}
