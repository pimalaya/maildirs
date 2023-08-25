pub mod entry;
pub mod flag;
pub mod folder;
mod validate;

#[cfg(unix)]
use std::os::unix::fs::MetadataExt;
#[cfg(windows)]
use std::os::windows::fs::MetadataExt;
use std::{
    fs::{self, File, OpenOptions},
    io::{self, ErrorKind, Write},
    path::{Path, PathBuf},
    process, str,
    sync::atomic::{AtomicUsize, Ordering},
    time::{self, SystemTime, UNIX_EPOCH},
};

use entry::{MailEntries, MailEntry};
use folder::Folders;
use gethostname::gethostname;
use thiserror::Error;

const CUR: &str = "cur";
const NEW: &str = "new";
const TMP: &str = "tmp";
#[cfg(unix)]
const SEP: &str = ":";
#[cfg(windows)]
const SEP: &str = ";";

static COUNTER: AtomicUsize = AtomicUsize::new(0);

#[non_exhaustive]
#[derive(Debug, Error)]
pub enum Error {
    #[error("cannot find email {0}")]
    FindEmailError(String),
    #[error("cannot get invalid file name from email {0}")]
    GetEmailFileNameError(String),
    #[error("cannot copy email to the same path {0}")]
    CopyEmailSamePathError(PathBuf),
    #[error("invalid id {0}")]
    InvalidIdError(String),
    #[error("invalid folder {0}")]
    InvalidFolderError(String),
    #[error("invalid flag {0}")]
    InvalidFlagError(char),
    #[error("{0} already exists")]
    AlreadyExistsError(PathBuf),
    #[error(transparent)]
    Utf8Error(#[from] str::Utf8Error),
    #[error(transparent)]
    IoError(#[from] io::Error),
    #[error(transparent)]
    SystemTimeError(#[from] time::SystemTimeError),
}

/// The main entry point for this library. This struct can be
/// instantiated from a path using the `from` implementations.
/// The path passed in to the `from` should be the root of the
/// maildir (the folder containing `cur`, `new`, and `tmp`).
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

    /// Returns an iterator over the maildir subdirectories. The order of
    /// subdirectories in the iterator is not specified, and is not guaranteed
    /// to be stable over multiple invocations of this method.
    pub fn folders(&self) -> Folders {
        Folders::new(&self.root)
    }

    /// Returns the number of messages found inside the `new` folder.
    pub fn count_new(&self) -> usize {
        self.new.read_dir().map(|r| r.count()).unwrap_or_else(|_| 0)
    }

    /// Returns the number of messages found inside the `cur` folder.
    pub fn count_cur(&self) -> usize {
        self.cur.read_dir().map(|r| r.count()).unwrap_or_else(|_| 0)
    }

    /// Returns the number of messages found inside the `tmp` folder.
    pub fn count_tmp(&self) -> usize {
        self.tmp.read_dir().map(|r| r.count()).unwrap_or_else(|_| 0)
    }

    /// Returns an iterator over the messages inside the `new` maildir folder.
    /// The order of messages in the iterator is not specified, and is not
    /// guaranteed to be stable over multiple invocations of this method.
    pub fn list_new(&self) -> io::Result<MailEntries> {
        MailEntries::new(&self.new)
    }

    /// Returns an iterator over the messages inside the `cur` maildir folder.
    /// The order of messages in the iterator is not specified, and is not
    /// guaranteed to be stable over multiple invocations of this method.
    pub fn list_cur(&self) -> io::Result<MailEntries> {
        MailEntries::new(&self.cur)
    }

    /// Copies a message from the current maildir to the targetted maildir.
    pub fn copy_to(&self, id: &str, target: &Maildir) -> Result<(), Error> {
        let entry = self
            .find(id)
            .ok_or_else(|| Error::FindEmailError(id.to_owned()))?;
        let filename = entry
            .path()
            .file_name()
            .ok_or_else(|| Error::GetEmailFileNameError(id.to_owned()))?;

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
            .ok_or_else(|| Error::GetEmailFileNameError(id.to_owned()))?;
        fs::rename(entry.path(), target.path().join("cur").join(filename))?;
        Ok(())
    }

    /// Tries to find the message with the given id in the maildir.
    ///
    /// This searches both `new` and `cur`, but does not traverse subfolders.
    pub fn find(&self, id: &str) -> Option<MailEntry> {
        self.list_new()
            .ok()?
            .chain(self.list_cur().ok()?)
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
    /// folder. Does not create the neccessary directories, so if in doubt call
    /// `create_dirs` before using `store_new`. Returns the Id of the inserted
    /// message on success.
    pub fn store_new(&self, data: &[u8]) -> Result<MailEntry, Error> {
        self.store(data, true, None)
    }

    /// Stores the given message data as a new message file in the Maildir `cur`
    /// folder.
    /// e.g. at <https://cr.yp.to/proto/maildir.html> or
    /// <http://www.courier-mta.org/maildir.html>. Returns the Id of the
    /// inserted message on success.
    pub fn store_cur(&self, data: &[u8]) -> Result<MailEntry, Error> {
        self.store(data, false, None)
    }

    fn store(&self, data: &[u8], new: bool, id: Option<String>) -> Result<MailEntry, Error> {
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

    #[cfg(unix)]
    let size = meta.size();
    #[cfg(windows)]
    let size = meta.file_size();

    let hostname = gethostname()
        .into_string()
        .expect("hostname is not valid UTF-8. how the fuck did you achieve that?");

    Ok(format!(
        "{}V{dev}I{ino}.{hostname},S={size}",
        generate_tmp_id()
    ))
}

// TODO: Folder name validation
