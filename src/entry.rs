#[cfg(unix)]
use std::os::unix::prelude::OsStrExt;
#[cfg(windows)]
use std::os::windows::prelude::OsStrExt;
use std::{
    collections::HashSet,
    fs::{self, read, read_dir, ReadDir},
    io,
    path::{Path, PathBuf},
};

use crate::{validate::validate_id, Error, Flag, CUR, NEW, SEP, TMP};

/// A struct representing a single email message inside the maildir.
///
/// No parsing is done. This struct only holds the path to the message file,
/// and handles file system operations. The struct can only be created by
/// methods in [`Maildir`](crate::Maildir).
#[derive(Debug)]
pub struct MailEntry {
    id: String,
    flags: HashSet<Flag>,
    path: PathBuf,
}

impl MailEntry {
    fn from_path<P: AsRef<Path>>(path: P) -> Result<Self, Error> {
        let path = path.as_ref();
        let filename = std::str::from_utf8(
            path.file_name()
                .ok_or(Error::InvalidFilenameError(
                    path.to_string_lossy().to_string(),
                ))?
                .as_bytes(),
        )?;

        let mut split = filename.split(SEP).peekable();
        let mut id = split.next().unwrap().to_string();
        while let Some(s) = split.next() {
            if split.peek().is_some() {
                id.push_str(s);
            }
        }

        let flags = filename
            .split(&format!("{SEP}2,")) // We are ignoring any experimental info (marked `:1,`)
            .last() // Allow the occurence of `:2,` in the filename
            .unwrap_or("")
            .chars()
            .map(TryFrom::try_from)
            .filter_map(Result::ok)
            .collect();

        Ok(MailEntry {
            id,
            flags,
            path: path.to_path_buf(),
        })
    }

    pub(crate) fn create<P: AsRef<Path>, S: ToString>(
        id: S,
        path: P,
        data: &[u8],
    ) -> Result<Self, Error> {
        let path = path.as_ref();
        fs::write(path, data)?;
        Ok(MailEntry {
            id: id.to_string(),
            flags: HashSet::new(),
            path: path.to_path_buf(),
        })
    }

    fn update(&mut self) -> Result<(), Error> {
        let new_file_name = format!(
            "{id}{SEP}2,{flags}",
            id = self.id,
            flags = self.flags_to_string()
        );

        let prev_path = self.path.clone();
        let new_path = self.path.with_file_name(new_file_name);

        if new_path.exists() {
            return Err(Error::AlreadyExistsError(new_path));
        }

        self.path = new_path;
        Ok(fs::rename(prev_path, &self.path)?)
    }

    /// Get the unique identifier of the email message.
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Set the unique identifier of the email message.
    ///
    /// This also updates the path to the email message and renames the file on
    /// the file system.
    ///
    /// # Errors
    ///
    /// This method will return an error if the new ID is invalid or if there
    /// was an error renaming the file on the file system.
    pub fn set_id<S: ToString>(&mut self, id: S) -> Result<(), Error> {
        self.id = validate_id(id.to_string())?;
        self.update()
    }

    /// Get the path to the email message.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Moves the mail entry
    fn move_to(&mut self, folder: &str) -> Result<(), Error> {
        let parent = self
            .path
            .parent()
            .ok_or_else(|| Error::NoParentError(self.path.clone()))?;

        if parent.file_name() == Some(folder.as_ref()) {
            return Ok(());
        }

        let new_path = parent
            .parent()
            .ok_or_else(|| Error::NoParentError(parent.to_path_buf()))?
            .join(folder)
            // We can unwrap here because we know that the parent is a directory
            .join(self.path.file_name().unwrap());

        fs::rename(&self.path, &new_path)?;
        self.path = new_path;

        Ok(())
    }

    /// Moves the email message to the `new` directory.
    pub fn move_to_new(&mut self) -> Result<(), Error> {
        self.move_to(NEW)
    }

    /// Moves the email message to the `cur` directory.
    pub fn move_to_cur(&mut self) -> Result<(), Error> {
        self.move_to(CUR)
    }

    /// Moves the email message to the `tmp` directory.
    pub fn move_to_tmp(&mut self) -> Result<(), Error> {
        self.move_to(TMP)
    }

    /// Get the flags of the email message.
    pub fn flags(&self) -> impl Iterator<Item = &Flag> {
        self.flags.iter()
    }

    /// Get the flags of the email message as a string.
    pub fn flags_to_string(&self) -> String {
        let mut flags: Vec<&str> = self.flags().map(AsRef::as_ref).collect();
        flags.sort();
        flags.join("")
    }

    /// Set a flag on the email message.
    ///
    /// This also updates the path to the email message and renames the file on
    /// the file system.
    ///
    /// # Errors
    ///
    /// This method will return an error if there was an error renaming the
    /// file.
    pub fn set_flag(&mut self, flag: Flag) -> Result<(), Error> {
        if self.flags.insert(flag) {
            self.update()?;
        }
        Ok(())
    }

    /// Unset a flag on the email message.
    ///
    /// This also updates the path to the email message and renames the file on
    /// the file system.
    ///
    /// # Errors
    ///
    /// This method will return an error if there was an error renaming the
    /// file.
    pub fn unset_flag(&mut self, flag: Flag) -> Result<(), Error> {
        if self.flags.remove(&flag) {
            self.update()?;
        }
        Ok(())
    }

    /// Returns `true` if the email message has the supplied flag
    pub fn has_flag(&self, flag: Flag) -> bool {
        self.flags.contains(&flag)
    }

    /// Get the raw bytes of the email message.
    ///
    /// # Errors
    ///
    /// This method will return an error if the email message could not be read
    /// from the file system. This could be because the path does not exists, or
    /// if there was another read error (e.g. permission denied.)
    pub fn to_bytes(&self) -> io::Result<Vec<u8>> {
        read(&self.path)
    }
}

/// An iterator over email messages in a maildir (either from `cur`, `new` or
/// `tmp`).
///
/// This iterator produces a `Result<MailEntry>`, which can be an `Err` if an
/// error was encountered while trying to read file system properties on a
/// particular entry, or if an invalid file was found in the maildir. Files
/// starting with a dot (.) character in the maildir folder are ignored.
pub struct MailEntries {
    readdir: Option<ReadDir>,
    move_to_cur: bool,
}

impl MailEntries {
    pub(crate) fn new<P: AsRef<Path>>(path: P, move_to_cur: bool) -> MailEntries {
        MailEntries {
            readdir: read_dir(path).ok(),
            move_to_cur,
        }
    }
}

impl Iterator for MailEntries {
    type Item = Result<MailEntry, Error>;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(ref mut readdir) = self.readdir {
            for entry in readdir {
                let path = match entry {
                    Err(e) => return Some(Err(e.into())),
                    Ok(e) => e.path(),
                };

                if path.is_dir()
                    || path
                        .file_name()
                        .map_or(true, |n| n.to_string_lossy().starts_with('.'))
                {
                    continue;
                }

                let mut entry = MailEntry::from_path(path);

                if self.move_to_cur {
                    if let Ok(ref mut entry) = entry {
                        if let Err(e) = entry.move_to_cur() {
                            return Some(Err(e));
                        }
                    }
                }

                return Some(entry);
            }
        }

        None
    }
}
