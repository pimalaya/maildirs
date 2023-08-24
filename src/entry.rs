use std::{
    fs::{read, read_dir, File, ReadDir},
    io::{BufRead, BufReader},
    path::{Path, PathBuf},
};

use crate::Result;

/// This struct represents a single email message inside
/// the maildir. Creation of the struct does not automatically
/// load the content of the email file into memory - however,
/// that may happen upon calling functions that require parsing
/// the email.
pub struct MailEntry {
    id: String,
    flags: String,
    path: PathBuf,
    headers: Vec<u8>,
}

impl MailEntry {
    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn flags(&self) -> &str {
        &self.flags
    }

    pub fn is_draft(&self) -> bool {
        self.flags.contains('D')
    }

    pub fn is_flagged(&self) -> bool {
        self.flags.contains('F')
    }

    pub fn is_passed(&self) -> bool {
        self.flags.contains('P')
    }

    pub fn is_replied(&self) -> bool {
        self.flags.contains('R')
    }

    pub fn is_seen(&self) -> bool {
        self.flags.contains('S')
    }

    pub fn is_trashed(&self) -> bool {
        self.flags.contains('T')
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn headers(&self) -> &[u8] {
        &self.headers
    }

    pub fn body(&self) -> Result<Vec<u8>> {
        Ok(read(&self.path)?)
    }
}

/// An iterator over the email messages in a particular
/// maildir subfolder (either `cur` or `new`). This iterator
/// produces a `Result<MailEntry>`, which can be an
/// `Err` if an error was encountered while trying to read
/// file system properties on a particular entry, or if an
/// invalid file was found in the maildir. Files starting with
/// a dot (.) character in the maildir folder are ignored.
pub struct MailEntries {
    readdir: ReadDir,
}

impl MailEntries {
    pub(crate) fn new<P: AsRef<Path>>(path: P) -> Result<MailEntries> {
        Ok(MailEntries {
            readdir: read_dir(path)?,
        })
    }
}

impl Iterator for MailEntries {
    type Item = Result<MailEntry>;

    fn next(&mut self) -> Option<Self::Item> {
        for entry in self.readdir.by_ref() {
            let path = match entry {
                Err(e) => return Some(Err(e.into())),
                Ok(e) => e.path(),
            };

            if path.is_dir() || path.starts_with(".") {
                continue;
            }

            let headers = match read_headers(&path) {
                Ok(headers) => headers,
                Err(e) => return Some(Err(e)),
            };

            return Some(Ok(MailEntry {
                id: path.to_string_lossy().to_string(),
                flags: "".to_string(),
                path: path.to_path_buf(),
                headers,
            }));
        }

        None
    }
}

/// Reads the headers of a MIME message into a buffer.
fn read_headers(path: impl AsRef<Path>) -> Result<Vec<u8>> {
    let mut buffer = Vec::new();
    for line in BufReader::new(File::open(path.as_ref())?).split(b'\n') {
        let line = line?;
        if line.is_empty() || line.last() == Some(&b'\r') {
            break;
        }
        buffer.extend(line);
    }
    Ok(buffer)
}
