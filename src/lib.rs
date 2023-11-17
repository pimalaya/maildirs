mod entry;
mod error;
mod flag;
mod validate;

#[cfg(unix)]
use std::os::unix::fs::MetadataExt;
#[cfg(windows)]
use std::os::windows::fs::MetadataExt;
use std::{
    fs::{self, File, OpenOptions, ReadDir},
    io::{self, ErrorKind, Write},
    path::{Path, PathBuf},
    process, str,
    sync::atomic::{AtomicUsize, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

pub use entry::{MailEntries, MailEntry};
pub use error::Error;
pub use flag::Flag;
use gethostname::gethostname;

const CUR: &str = "cur";
const NEW: &str = "new";
const TMP: &str = "tmp";
#[cfg(unix)]
const SEP: &str = ":";
#[cfg(windows)]
const SEP: &str = ";";

static COUNTER: AtomicUsize = AtomicUsize::new(0);

/// The main entry point for this library. This struct can be
/// instantiated from a path using the `from` implementations.
/// The path passed in to the `from` should be the root of the
/// maildir (the folder containing `cur`, `new`, and `tmp`).
#[derive(Debug)]
pub struct Maildir {
    root: PathBuf,
    cur: PathBuf,
    new: PathBuf,
    tmp: PathBuf,
}

impl Maildir {
    /// Creates a new maildir at the given path. This will ensure
    /// all the necessary subfolders exist.
    ///
    /// # Errors
    ///
    /// This function will return an error if it cannot create
    /// the necessary subfolders.
    pub fn new<P: AsRef<Path>>(path: P) -> Result<Self, Error> {
        let maildir = Maildir::from(path);
        maildir.ensure_dirs()?;
        maildir.clean_tmp()?;
        Ok(maildir)
    }

    /// Returns the path of the maildir root.
    pub fn path(&self) -> &Path {
        &self.root
    }

    /// Ensures that the necessary subfolders exist.
    fn ensure_dirs(&self) -> Result<(), Error> {
        for dir in &[&self.cur, &self.new, &self.tmp] {
            if !dir.exists() {
                fs::create_dir_all(dir)?;
            }
        }
        Ok(())
    }

    /// Remove any files in the tmp folder that are older than 36 hours.
    pub fn clean_tmp(&self) -> Result<(), Error> {
        for entry in fs::read_dir(&self.tmp)? {
            let path = entry?.path();
            // If the file is older than 36 hours, delete it
            if path.is_file() && path.metadata()?.modified()?.elapsed()?.as_secs() > 36 * 60 * 60 {
                fs::remove_file(path)?;
            }
        }
        Ok(())
    }

    /// Creates a new folder in this maildir.
    ///
    /// If the maildir is already a subfolder of another maildir, the new folder
    /// will be created as a subfolder of the parent maildir.
    ///
    /// # Errors
    ///
    /// This method returns an error if the folder name is invalid, or if there
    /// was an error from the file system when creating the folder and its
    /// contents.
    pub fn create_folder(&self, folder: &str) -> Result<Maildir, Error> {
        validate::validate_folder(folder)?;
        let path = if self.root.join("maildirfolder").exists() {
            self.root.parent().unwrap().join(format!(
                "{}.{folder}",
                self.root.file_name().unwrap().to_string_lossy()
            ))
        } else {
            self.root.join(format!(".{folder}"))
        };

        fs::create_dir_all(&path)?;
        fs::write(path.join("maildirfolder"), "")?;

        Maildir::new(path)
    }

    /// Returns an iterator over the maildir subdirectories.
    ///
    /// The order of subdirectories in the iterator is not specified, and is not
    /// guaranteed to be stable over multiple invocations of this method. Note
    /// also that it is assumed that the maildir root exists and is readable by
    /// the running process. The returned iterator will be empty if that is not
    /// the case.
    pub fn folders(&self) -> Maildirs {
        // TODO: Ensure subfolders function properly as well
        Maildirs::new(&self.root)
    }

    /// Returns the number of messages found inside the `new` folder.
    pub fn count_new(&self) -> usize {
        MailEntries::new(&self.new, false).count()
    }

    /// Returns the number of messages found inside the `cur` folder.
    pub fn count_cur(&self) -> usize {
        MailEntries::new(&self.cur, false)
            .inspect(|e| println!("{:?}", e))
            .count()
    }

    /// Returns the number of messages found inside the `tmp` folder.
    pub fn count_tmp(&self) -> usize {
        MailEntries::new(&self.tmp, false).count()
    }

    /// Returns an iterator over the messages inside the `new` maildir folder.
    ///
    /// This will move all mail entries it encounters into the `cur` directory.
    /// If that is undesired, have look at the [`Maildir::peek_new`] method.
    ///
    /// The order of messages in the iterator is not specified, and is not
    /// guaranteed to be stable over multiple invocations of this method. Note
    /// also that it is assumed that the `new` folder exists and is readable by
    /// the running process. The returned iterator will be empty if that is not
    /// the case.
    pub fn list_new(&self) -> MailEntries {
        MailEntries::new(&self.new, true)
    }

    /// Returns an iterator over the messages inside the `cur` maildir folder.
    ///
    /// The order of messages in the iterator is not specified, and is not
    /// guaranteed to be stable over multiple invocations of this method. Note
    /// also that it is assumed that the `cur` folder exists and is readable by
    /// the running process. The returned iterator will be empty if that is not
    /// the case.
    pub fn list_cur(&self) -> MailEntries {
        MailEntries::new(&self.cur, false)
    }

    /// Returns an iterator over the messages inside the `tmp` maildir folder.
    ///
    /// The order of messages in the iterator is not specified, and is not
    /// guaranteed to be stable over multiple invocations of this method. Note
    /// also that it is assumed that the `tmp` folder exists and is readable by
    /// the running process. The returned iterator will be empty if that is not
    /// the case.
    pub fn list_tmp(&self) -> MailEntries {
        MailEntries::new(&self.tmp, false)
    }

    /// Returns an iterator over the messages inside the `new` maildir folder,
    /// without moving them to `cur`.
    ///
    /// The order of messages in the iterator is not specified, and is not
    /// guaranteed to be stable over multiple invocations of this method. Note
    /// also that it is assumed that the `new` folder exists and is readable by
    /// the running process. The returned iterator will be empty if that is not
    /// the case.
    pub fn peek_new(&self) -> MailEntries {
        MailEntries::new(&self.new, true)
    }

    /// Copies a message from the current maildir to the targetted maildir.
    pub fn copy_to(&self, id: &str, target: &Maildir) -> Result<(), Error> {
        let entry = self
            .find(id)
            .ok_or_else(|| Error::FindEmailError(id.to_owned()))?;
        let filename = entry
            .path()
            .file_name()
            .ok_or_else(|| Error::InvalidFilenameError(id.to_owned()))?;

        let src_path = entry.path();
        let dst_path = target.path().join("cur").join(filename);
        if src_path == dst_path {
            return Err(Error::CopyEmailSamePathError(dst_path));
        }

        fs::copy(src_path, dst_path)?;
        Ok(())
    }

    /// Moves a message from the current maildir to the targetted maildir.
    pub fn move_to(&self, id: &str, target: &Maildir) -> Result<(), Error> {
        let entry = self
            .find(id)
            .ok_or_else(|| Error::FindEmailError(id.to_owned()))?;
        let filename = entry
            .path()
            .file_name()
            .ok_or_else(|| Error::InvalidFilenameError(id.to_owned()))?;
        fs::rename(entry.path(), target.path().join("cur").join(filename))?;
        Ok(())
    }

    /// Tries to find the message with the given id in the maildir.
    ///
    /// This searches both `new` and `cur`, but does not traverse subfolders.
    pub fn find(&self, id: &str) -> Option<MailEntry> {
        self.list_new()
            .chain(self.list_cur())
            .filter_map(Result::ok)
            .find(|entry| entry.id() == id)
    }

    /// Deletes the message with the given id in the maildir.
    ///
    /// This searches both the `new` and the `cur` folders, and deletes the file
    /// from the filesystem.
    ///
    /// # Errors
    ///
    /// This method returns an error if no message was found with the given id,
    /// or if there was an error when deleting the file.
    pub fn delete(&self, id: &str) -> Result<(), Error> {
        match self.find(id) {
            Some(m) => Ok(fs::remove_file(m.path())?),
            None => Err(Error::FindEmailError(id.to_owned())),
        }
    }

    /// Stores the given message data as a new message file in the Maildir `new`
    /// folder.
    pub fn store_new(&self, data: &[u8]) -> Result<MailEntry, Error> {
        self.store(data, true, None)
    }

    /// Stores the given message data as a new message file in the Maildir `cur`
    /// folder.
    pub fn store_cur(&self, data: &[u8]) -> Result<MailEntry, Error> {
        self.store(data, false, None)
    }

    fn store(&self, data: &[u8], new: bool, id: Option<String>) -> Result<MailEntry, Error> {
        self.ensure_dirs()?;

        // loop when conflicting filenames occur, as described at
        // <http://www.courier-mta.org/maildir.html> this assumes that pid and
        // hostname don't change.
        let mut tmp_file;
        let mut tmp_path = self.tmp.clone();
        loop {
            tmp_path.push(generate_tmp_id());

            match OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&tmp_path)
            {
                Ok(f) => {
                    tmp_file = f;
                    break;
                }
                Err(err) => {
                    if err.kind() != ErrorKind::AlreadyExists {
                        return Err(err.into());
                    }
                    tmp_path.pop();
                }
            }
        }

        /// At this point, `file` is our new file at `tmppath`. If we
        /// leave the scope of this function prior to
        /// successfully writing the file to its final
        /// location, we need to ensure that we remove
        /// the temporary file. This struct takes care
        /// of that detail.
        struct RemoveOnDrop {
            path_to_remove: PathBuf,
        }

        impl Drop for RemoveOnDrop {
            fn drop(&mut self) {
                fs::remove_file(&self.path_to_remove).ok();
            }
        }

        // Ensure that we remove the temporary file on failure
        let _remove_guard = RemoveOnDrop {
            path_to_remove: tmp_path.clone(),
        };

        tmp_file.write_all(data)?;
        tmp_file.sync_all()?;

        let id = id.map_or_else(|| generate_id(tmp_file), Ok)?;

        let mut new_path = self.root.clone();
        if new {
            new_path.push(NEW);
            new_path.push(&id);
        } else {
            new_path.push(CUR);
            new_path.push(format!("{id}{SEP}2"));
        }

        fs::rename(&tmp_path, &new_path)?;
        MailEntry::create(id, new_path, data)
    }
}

impl<P: AsRef<Path>> From<P> for Maildir {
    fn from(p: P) -> Maildir {
        Maildir {
            root: p.as_ref().to_path_buf(),
            cur: p.as_ref().join(CUR),
            new: p.as_ref().join(NEW),
            tmp: p.as_ref().join(TMP),
        }
    }
}

/// An iterator of maildirs subdirectories, typically used to iterate over
/// subfolders.
///
/// This iterator produces an [`io::Result<Maildir>`], which can be an `Err` if
/// an error was encountered while trying to read file system properties on a
/// particular entry. Only subdirectories starting with a single period are
/// included.
pub struct Maildirs {
    readdir: Option<ReadDir>,
}

impl Maildirs {
    pub(crate) fn new<P: AsRef<Path>>(path: P) -> Maildirs {
        Maildirs {
            readdir: fs::read_dir(path).ok(),
        }
    }
}

impl Iterator for Maildirs {
    type Item = io::Result<Maildir>;

    fn next(&mut self) -> Option<io::Result<Maildir>> {
        if let Some(ref mut readdir) = self.readdir {
            for entry in readdir {
                let path = match entry {
                    Err(e) => return Some(Err(e)),
                    Ok(e) => e.path(),
                };

                let filename = path.file_name()?.to_string_lossy();

                if !filename.starts_with('.') || filename.starts_with("..") || !path.is_dir() {
                    continue;
                }

                return Some(Ok(Maildir::from(path)));
            }
        }

        None
    }
}

fn generate_tmp_id() -> String {
    let ts = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
    let secs = ts.as_secs();
    let nanos = ts.subsec_nanos();
    let counter = COUNTER.fetch_add(1, Ordering::SeqCst);
    format!(
        "{secs}.#{counter:x}M{nanos}P{pid}",
        secs = secs,
        counter = counter,
        nanos = nanos,
        pid = process::id()
    )
}

fn generate_id(file: File) -> Result<String, Error> {
    let meta = file.metadata()?;

    #[cfg(unix)]
    let dev = meta.dev();
    #[cfg(windows)]
    let dev: u64 = 0;

    #[cfg(unix)]
    let ino = meta.ino();
    #[cfg(windows)]
    let ino: u64 = 0;

    let hostname = gethostname()
        .into_string()
        .expect("hostname is not valid UTF-8. how the fuck did you achieve that?");

    Ok(format!("{}V{dev}I{ino}.{hostname}", generate_tmp_id()))
}
