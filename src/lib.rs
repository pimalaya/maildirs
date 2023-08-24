pub mod entry;
pub mod folder;

#[cfg(unix)]
use std::os::unix::fs::MetadataExt;
#[cfg(windows)]
use std::os::windows::fs::MetadataExt;
use std::{
    fs::{self, File, OpenOptions},
    io::{self, ErrorKind, Write},
    path::{Path, PathBuf},
    process, result,
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

static COUNTER: AtomicUsize = AtomicUsize::new(0);

#[cfg(unix)]
const INFORMATIONAL_SUFFIX_SEPARATOR: &str = ":";
#[cfg(windows)]
const INFORMATIONAL_SUFFIX_SEPARATOR: &str = ";";

#[non_exhaustive]
#[derive(Debug, Error)]
pub enum Error {
    #[error("cannot find email {0}")]
    FindEmailError(String),
    #[error("cannot get invalid file name from email {0}")]
    GetEmailFileNameError(String),
    #[error("cannot copy email to the same path {0}")]
    CopyEmailSamePathError(PathBuf),
    #[error("cannot get subfolder name")]
    GetSubfolderNameError,
    #[error(transparent)]
    IoError(#[from] io::Error),
    #[error(transparent)]
    SystemTimeError(#[from] time::SystemTimeError),
}

type Result<T> = result::Result<T, Error>;

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
    pub fn new<P: AsRef<Path>>(path: P) -> Result<Self> {
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
    fn ensure_dirs(&self) -> Result<()> {
        for dir in &[&self.cur, &self.new, &self.tmp] {
            if !dir.exists() {
                fs::create_dir_all(dir)?;
            }
        }
        Ok(())
    }

    /// Remove any files in the tmp folder that are older than 36 hours.
    pub fn clean_tmp(&self) -> Result<()> {
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
    pub fn create_folder(&self, folder: &str) -> Result<Maildir> {
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
    pub fn list_new(&self) -> Result<MailEntries> {
        MailEntries::new(&self.new)
    }

    /// Returns an iterator over the messages inside the `cur` maildir folder.
    /// The order of messages in the iterator is not specified, and is not
    /// guaranteed to be stable over multiple invocations of this method.
    pub fn list_cur(&self) -> Result<MailEntries> {
        MailEntries::new(&self.cur)
    }

    /// Copies a message from the current maildir to the targetted maildir.
    pub fn copy_to(&self, id: &str, target: &Maildir) -> Result<()> {
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
    pub fn move_to(&self, id: &str, target: &Maildir) -> Result<()> {
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

    /// Tries to find the message with the given id in the maildir. This
    /// searches both the `new` and the `cur` folders.
    pub fn find(&self, id: &str) -> Option<MailEntry> {
        let filter = |entry: &Result<MailEntry>| match *entry {
            Err(_) => false,
            Ok(ref e) => e.id() == id,
        };

        self.list_new()
            .ok()?
            .chain(self.list_cur().ok()?)
            .find(&filter)
            .and_then(|e| e.ok())
    }

    fn update_flags<F>(&self, id: &str, flag_op: F) -> Result<()>
    where
        F: Fn(&str) -> String,
    {
        let filter = |entry: &Result<MailEntry>| match *entry {
            Err(_) => false,
            Ok(ref e) => e.id() == id,
        };

        match self.list_cur()?.find(&filter).map(|e| e.unwrap()) {
            Some(m) => {
                let src = m.path();
                let mut dst = m.path().to_path_buf();
                dst.pop();
                dst.push(format!(
                    "{}{}2,{}",
                    m.id(),
                    INFORMATIONAL_SUFFIX_SEPARATOR,
                    flag_op(m.flags())
                ));
                fs::rename(src, dst)?;
                Ok(())
            }
            None => Err(Error::FindEmailError(id.to_owned())),
        }
    }

    /// Updates the flags for the message with the given id in the maildir. This
    /// only searches the `cur` folder, because that's the folder where messages
    /// have flags. Returns an error if the message was not found. All existing
    /// flags are overwritten with the new flags provided.
    pub fn set_flags(&self, id: &str, flags: &str) -> Result<()> {
        self.update_flags(id, |_old_flags| normalize_flags(flags))
    }

    /// Adds the given flags to the message with the given id in the maildir.
    /// This only searches the `cur` folder, because that's the folder where
    /// messages have flags. Returns an error if the message was not found.
    /// Flags are deduplicated, so setting a already-set flag has no effect.
    pub fn add_flags(&self, id: &str, flags: &str) -> Result<()> {
        let flag_merge = |old_flags: &str| {
            let merged = String::from(old_flags) + flags;
            normalize_flags(&merged)
        };
        self.update_flags(id, flag_merge)
    }

    /// Removes the given flags to the message with the given id in the maildir.
    /// This only searches the `cur` folder, because that's the folder where
    /// messages have flags. Returns an error if the message was not found. If
    /// the message doesn't have the flag(s) to be removed, those flags are
    /// ignored.
    pub fn remove_flags(&self, id: &str, flags: &str) -> Result<()> {
        let flag_strip =
            |old_flags: &str| old_flags.chars().filter(|c| !flags.contains(*c)).collect();
        self.update_flags(id, flag_strip)
    }

    /// Deletes the message with the given id in the maildir. This searches both
    /// the `new` and the `cur` folders, and deletes the file from the
    /// filesystem. Returns an error if no message was found with the given id.
    pub fn delete(&self, id: &str) -> Result<()> {
        match self.find(id) {
            Some(m) => Ok(fs::remove_file(m.path())?),
            None => Err(Error::FindEmailError(id.to_owned())),
        }
    }

    /// Stores the given message data as a new message file in the Maildir `new`
    /// folder. Does not create the neccessary directories, so if in doubt call
    /// `create_dirs` before using `store_new`. Returns the Id of the inserted
    /// message on success.
    pub fn store_new(&self, data: &[u8]) -> Result<String> {
        self.store(data, true)
    }

    /// Stores the given message data as a new message file in the Maildir `cur`
    /// folder, adding the given `flags` to it. The possible flags are explained
    /// e.g. at <https://cr.yp.to/proto/maildir.html> or
    /// <http://www.courier-mta.org/maildir.html>. Returns the Id of the
    /// inserted message on success.
    pub fn store_cur(&self, data: &[u8]) -> Result<String> {
        self.store(data, false)
    }

    fn store(&self, data: &[u8], new: bool) -> Result<String> {
        // loop when conflicting filenames occur, as described at
        // http://www.courier-mta.org/maildir.html this assumes that
        // pid and hostname don't change.
        let mut tmp_path = self.tmp.clone();
        let mut tmp_file;

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

        /// At this point, `file` is our new file at `tmppath`. If we leave the
        /// scope of this function prior to successfully writing the file to its
        /// final location, we need to ensure that we remove the temporary file.
        /// This struct takes care of that detail.
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

        let mut newpath = self.root.clone();
        newpath.push(if new { NEW } else { CUR });

        let mut id = generate_id(tmp_file)?;
        if !new {
            id.push_str(INFORMATIONAL_SUFFIX_SEPARATOR);
            id.push('2');
        }
        newpath.push(&id);

        fs::rename(&tmp_path, &newpath)?;
        Ok(id)
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

fn normalize_flags(flags: &str) -> String {
    let mut flags = flags.to_uppercase().bytes().collect::<Vec<_>>();
    flags.sort_unstable();
    flags.dedup();
    // SAFETY: we know that the bytes in `flags` are all valid UTF-8
    unsafe { String::from_utf8_unchecked(flags) }
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

fn generate_id(file: File) -> Result<String> {
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
