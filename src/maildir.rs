#[cfg(unix)]
use std::os::unix::fs::MetadataExt;
#[cfg(windows)]
use std::os::windows::fs::MetadataExt;
use std::{
    collections::HashSet,
    ffi::OsStr,
    fs::{self, File, OpenOptions},
    hash::{Hash, Hasher},
    io::{self, BufRead, BufReader, Write},
    path::{Component, Path, PathBuf},
    process,
    sync::atomic::{AtomicUsize, Ordering},
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use gethostname::gethostname;
use walkdir::WalkDir;

use crate::{Error, Flag, Result};

static NEW: &str = "new";
static CUR: &str = "cur";
static TMP: &str = "tmp";

#[cfg(unix)]
static DEFAULT_INFO_SEPARATOR: &str = ":";
#[cfg(windows)]
static DEFAULT_INFO_SEPARATOR: &str = ";";

static H_36: u64 = 36 * 60 * 60;

static COUNTER: AtomicUsize = AtomicUsize::new(0);

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MaildirBuilder {
    pub info_separator: &'static str,
}

impl MaildirBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_info_separator(&mut self, sep: &'static str) {
        self.info_separator = sep;
    }

    pub fn with_info_separator(mut self, sep: &'static str) -> Self {
        self.set_info_separator(sep);
        self
    }

    pub fn build(self, path: impl Into<PathBuf>) -> Maildir {
        let mdir = Maildir::from(path.into()).with_info_separator(self.info_separator);

        if let Ok(mut entries) = fs::read_dir(&mdir.tmp()) {
            let _ = entries.try_for_each(|entry| {
                let path = entry?.path();
                let metadata = path.metadata()?;

                if metadata.is_file() && metadata.modified()?.elapsed()?.as_secs() > H_36 {
                    fs::remove_file(path)?;
                }

                Result::Ok(())
            });
        }

        mdir
    }
}

impl Default for MaildirBuilder {
    fn default() -> Self {
        Self {
            info_separator: DEFAULT_INFO_SEPARATOR,
        }
    }
}

/// The mail directory.
///
/// A Maildir is a mail directory composed of a `new`, `cur` and `tmp`
/// subdirectories.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Maildir {
    /// The root path of the mail directory.
    root: PathBuf,

    cur: PathBuf,
    new: PathBuf,
    tmp: PathBuf,

    /// The Maildir entry id ←→ info separator.
    info_separator: &'static str,
}

impl Maildir {
    pub fn set_info_separator(&mut self, sep: &'static str) {
        self.info_separator = sep;
    }

    pub fn with_info_separator(mut self, sep: &'static str) -> Self {
        self.set_info_separator(sep);
        self
    }

    pub fn path(&self) -> &Path {
        &self.root
    }

    pub fn name(&self) -> Result<&str> {
        let file_name = self
            .root
            .file_name()
            .and_then(OsStr::to_str)
            .ok_or_else(|| Error::GetMaildirNameError(self.root.clone()))?;

        // remove potential periods in front of the name (this happens
        // when using Maildir++ format).
        Ok(file_name.trim_start_matches('.'))
    }

    pub fn cur(&self) -> &Path {
        &self.cur
    }

    pub fn new(&self) -> &Path {
        &self.new
    }

    pub fn tmp(&self) -> &Path {
        &self.tmp
    }

    pub fn exists(&self) -> bool {
        self.root.is_dir() && self.cur.is_dir() && self.new.is_dir() && self.tmp.is_dir()
    }

    pub fn create(&self) -> Result<()> {
        fs::create_dir(&self.root)?;

        fs::create_dir(&self.cur)?;
        fs::create_dir(&self.new)?;
        fs::create_dir(&self.tmp)?;

        Ok(())
    }

    pub fn create_all(&self) -> Result<()> {
        fs::create_dir_all(&self.root)?;

        fs::create_dir_all(&self.cur)?;
        fs::create_dir_all(&self.new)?;
        fs::create_dir_all(&self.tmp)?;

        Ok(())
    }

    pub fn remove(&self) -> Result<()> {
        fs::remove_dir(&self.cur)?;
        fs::remove_dir(&self.new)?;
        fs::remove_dir(&self.tmp)?;

        fs::remove_dir(&self.root)?;

        Ok(())
    }

    pub fn remove_all(&self) -> Result<()> {
        fs::remove_dir_all(&self.root)?;
        Ok(())
    }

    pub fn read(&self) -> Result<impl Iterator<Item = MaildirEntry> + '_> {
        Ok(fs::read_dir(&self.new)?
            .chain(fs::read_dir(&self.cur)?)
            .filter_map(|entry| entry.ok())
            .filter(|entry| entry.path().is_file())
            .filter(|entry| {
                entry
                    .file_name()
                    .to_str()
                    .map(|s| !s.starts_with("."))
                    .unwrap_or(false)
            })
            .map(|entry| MaildirEntry::new(entry.path()).with_info_separator(self.info_separator)))
    }

    pub fn find(&self, id: impl AsRef<str>) -> Result<Option<MaildirEntry>> {
        let id = id.as_ref();

        let mdir = fs::read_dir(&self.new)?
            .chain(fs::read_dir(&self.cur)?)
            .filter_map(|entry| entry.ok())
            .find_map(|entry| {
                if !entry.path().is_file() {
                    return None;
                }

                let entry =
                    MaildirEntry::new(entry.path()).with_info_separator(self.info_separator);

                if id != entry.id().ok()? {
                    return None;
                }

                Some(entry)
            });

        Ok(mdir)
    }

    pub fn get(&self, id: impl AsRef<str>) -> Result<MaildirEntry> {
        let id = id.as_ref();

        match self.find(id)? {
            Some(mdir) => Ok(mdir),
            None => Err(Error::GetMaildirEntryNotFoundError(id.to_owned())),
        }
    }

    pub fn write_new(&self, contents: impl AsRef<[u8]>) -> Result<MaildirEntry> {
        self.write(contents, None, true, None)
    }

    pub fn write_cur(
        &self,
        contents: impl AsRef<[u8]>,
        flags: impl IntoIterator<Item = Flag>,
    ) -> Result<MaildirEntry> {
        self.write(contents, flags, false, None)
    }

    fn write(
        &self,
        contents: impl AsRef<[u8]>,
        flags: impl IntoIterator<Item = Flag>,
        new: bool,
        id: Option<String>,
    ) -> Result<MaildirEntry> {
        // loop when conflicting filenames occur, as described at
        // <http://www.courier-mta.org/maildir.html> this assumes that
        // pid and hostname don't change.
        let (tmp_path, mut tmp_file) = loop {
            let path = self.tmp.join(generate_tmp_id());
            let open = OpenOptions::new().write(true).create_new(true).open(&path);

            match open {
                Ok(file) => {
                    break Result::Ok((path, file));
                }
                Err(err) if err.kind() == io::ErrorKind::AlreadyExists => {
                    thread::sleep(Duration::from_secs(2));
                    continue;
                }
                Err(err) => {
                    break Err(err.into());
                }
            }
        }?;

        tmp_file.write_all(contents.as_ref())?;
        tmp_file.sync_all()?;

        let id = id.map_or_else(|| generate_id(tmp_file), Ok)?;
        let next_parent_path = if new { &self.new } else { &self.cur };
        let next_path = next_parent_path.join(if new {
            id
        } else {
            self.format_file_name(id, flags.into_iter().collect())
        });

        fs::rename(tmp_path, &next_path)?;

        let entry = fs::read_dir(next_parent_path)?
            .filter_map(|entry| entry.ok())
            .find(|entry| entry.path() == next_path);

        match entry {
            Some(entry) => {
                let entry = MaildirEntry::new(entry.path());
                Ok(entry.with_info_separator(self.info_separator))
            }
            None => Err(Error::FindNewMaildirEntryError(next_path)),
        }
    }

    fn format_file_name(&self, id: String, flags: HashSet<Flag>) -> String {
        format_file_name(self.info_separator, id, flags)
    }
}

impl Hash for Maildir {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.root.hash(state);
    }
}

impl<P: Into<PathBuf>> From<P> for Maildir {
    fn from(root: P) -> Self {
        let root = root.into();

        let new = root.join(NEW);
        let cur = root.join(CUR);
        let tmp = root.join(TMP);

        Self {
            root,
            new,
            cur,
            tmp,
            info_separator: DEFAULT_INFO_SEPARATOR,
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

    let hostname = gethostname()
        .into_string()
        .expect("hostname is not valid UTF-8. how the fuck did you achieve that?");

    Ok(format!("{}V{dev}I{ino}.{hostname}", generate_tmp_id()))
}

// =============================== LIST ================================

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Maildirs {
    root: PathBuf,
    maildirpp: bool,
    info_separator: &'static str,
}

impl Maildirs {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self {
            root: path.into(),
            maildirpp: false,
            info_separator: DEFAULT_INFO_SEPARATOR,
        }
    }

    pub fn set_maildirpp(&mut self, maildirpp: bool) {
        self.maildirpp = maildirpp;
    }

    pub fn with_maildirpp(mut self, maildirpp: bool) -> Self {
        self.set_maildirpp(maildirpp);
        self
    }

    pub fn set_info_separator(&mut self, sep: &'static str) {
        self.info_separator = sep;
    }

    pub fn with_info_separator(mut self, sep: &'static str) -> Self {
        self.set_info_separator(sep);
        self
    }

    pub fn path(&self) -> &Path {
        &self.root
    }

    fn maildir(&self, name: impl AsRef<str>) -> Maildir {
        let path = if self.maildirpp {
            let mut path = self.root.clone();

            for component in PathBuf::from(name.as_ref()).components() {
                if let Component::Normal(component) = component {
                    let component = component.to_string_lossy();
                    path.push(format!(".{}", component.trim_start_matches('.')))
                }
            }

            path
        } else {
            self.root.join(name.as_ref())
        };

        MaildirBuilder::new()
            .with_info_separator(self.info_separator)
            .build(path)
    }

    pub fn create(&self, name: impl ToString) -> Result<Maildir> {
        let mdir = self.maildir(name.to_string());
        mdir.create_all()?;
        Ok(mdir)
    }

    pub fn find(&self, name: impl AsRef<str>) -> Option<Maildir> {
        Some(self.maildir(name)).filter(|mdir| mdir.exists())
    }

    pub fn get(&self, name: impl AsRef<str>) -> Result<Maildir> {
        let name = name.as_ref();

        match self.find(name) {
            Some(mdir) => Ok(mdir),
            None => Err(Error::GetMaildirByNameNotFoundError(name.to_owned())),
        }
    }

    pub fn iter(&self) -> impl Iterator<Item = MaildirsEntry> + '_ {
        WalkDir::new(&self.root)
            .follow_links(true)
            .into_iter()
            .filter_map(|entry| entry.ok())
            .filter(|entry| {
                entry
                    .file_name()
                    .to_str()
                    .map(|s| !self.maildirpp || s.starts_with("."))
                    .unwrap_or(false)
            })
            .filter_map(|entry| {
                let name = if self.maildirpp {
                    if entry.path() == self.root {
                        return Some(MaildirsEntry {
                            maildirpp: self.maildirpp,
                            maildir: Maildir::from(&self.root),
                            name: self.root.file_name()?.to_str()?.to_owned(),
                        });
                    }

                    let subpath = entry.path().strip_prefix(&self.root).unwrap();
                    let mut name = PathBuf::new();

                    for component in subpath.components() {
                        if let Component::Normal(component) = component {
                            let component = component.to_string_lossy();
                            name.push(component.trim_start_matches('.'))
                        }
                    }

                    name.to_str()?.to_owned()
                } else {
                    entry
                        .path()
                        .strip_prefix(&self.root)
                        .ok()?
                        .to_str()?
                        .to_owned()
                };

                Some(MaildirsEntry {
                    maildirpp: self.maildirpp,
                    maildir: Maildir::from(entry.into_path()),
                    name,
                })
            })
            .filter_map(|entry| {
                if entry.maildir.exists() {
                    Some(entry)
                } else {
                    None
                }
            })
    }

    pub fn remove(&self, name: impl AsRef<str>) -> Result<()> {
        let mdir = self.maildir(name);
        mdir.remove_all()?;
        Ok(())
    }
}

impl Hash for Maildirs {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.root.hash(state);
        self.maildirpp.hash(state);
    }
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct MaildirsEntry {
    pub maildirpp: bool,
    pub maildir: Maildir,
    pub name: String,
}

// =============================== ENTRY ================================

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MaildirEntry {
    path: PathBuf,
    info_separator: &'static str,
}

impl MaildirEntry {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            info_separator: DEFAULT_INFO_SEPARATOR,
        }
    }

    pub fn set_info_separator(&mut self, sep: &'static str) {
        self.info_separator = sep;
    }

    pub fn with_info_separator(mut self, sep: &'static str) -> Self {
        self.set_info_separator(sep);
        self
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn file_name(&self) -> Result<&str> {
        let Some(name) = self.path.file_name() else {
            let path = self.path.clone();
            return Err(Error::GetMaildirEntryFileNameError(path));
        };

        match std::str::from_utf8(name.as_encoded_bytes()) {
            Ok(name) => Ok(name),
            Err(err) => {
                let path = self.path.clone();
                Err(Error::GetInvalidMaildirEntryFileNameError(err, path))
            }
        }
    }

    pub fn id(&self) -> Result<&str> {
        let file_name = self.file_name()?;

        Ok(match file_name.rsplit_once(self.info_separator) {
            Some((id, _)) => id,
            None => file_name,
        })
    }

    pub fn read(&self) -> Result<Vec<u8>> {
        let contents = fs::read(&self.path)?;
        Ok(contents)
    }

    pub fn read_headers(&self) -> Result<Vec<u8>> {
        let file = File::open(&self.path)?;
        let mut reader = BufReader::new(file);
        let mut buffer = Vec::<u8>::new();
        let mut headers = Vec::<u8>::new();

        loop {
            match reader.read_until(b'\n', &mut buffer)? {
                0 => {
                    break;
                }
                1 if buffer[0] == b'\n' => {
                    headers.push(b'\n');
                    break;
                }
                2 if buffer[0] == b'\r' && buffer[1] == b'\n' => {
                    headers.extend([b'\r', b'\n']);
                    break;
                }
                _ => {
                    headers.extend(&buffer);
                    buffer.clear();
                }
            }
        }

        Ok(headers)
    }

    pub fn flags(&self) -> Result<HashSet<Flag>> {
        Ok(match self.file_name()?.rsplit_once(self.info_separator) {
            Some((_, flags)) => flags
                .chars()
                .map(TryFrom::try_from)
                // remove invalid flag chars, including the "2,"
                // located just after the info separator
                .filter_map(Result::ok)
                .collect(),
            None => HashSet::new(),
        })
    }

    pub fn has_trash_flag(&self) -> bool {
        match self.flags() {
            Ok(flags) => flags.contains(&Flag::Trashed),
            Err(_) => false,
        }
    }

    pub fn insert_flag(&mut self, flag: Flag) -> Result<()> {
        self.insert_flags(Some(flag))
    }

    pub fn insert_flags(&mut self, flags: impl IntoIterator<Item = Flag>) -> Result<()> {
        let (flags, changed) =
            flags
                .into_iter()
                .fold((self.flags()?, false), |(mut flags, changed), flag| {
                    let inserted = flags.insert(flag);
                    (flags, changed || inserted)
                });

        if changed {
            let prev_path = self.path();
            let next_path = prev_path.with_file_name(self.format_file_name(flags)?);

            fs::rename(prev_path, &next_path)?;
            self.path = next_path;
        }

        Ok(())
    }

    pub fn update_flags(&mut self, flags: impl IntoIterator<Item = Flag>) -> Result<()> {
        let prev_path = self.path();
        let next_path =
            prev_path.with_file_name(self.format_file_name(flags.into_iter().collect())?);

        fs::rename(prev_path, &next_path)?;
        self.path = next_path;

        Ok(())
    }

    pub fn remove_flag(&mut self, flag: Flag) -> Result<()> {
        self.remove_flags(Some(flag))
    }

    pub fn remove_flags<'a>(&mut self, flags: impl IntoIterator<Item = Flag>) -> Result<()> {
        let (flags, changed) =
            flags
                .into_iter()
                .fold((self.flags()?, false), |(mut flags, changed), ref flag| {
                    let removed = flags.remove(flag);
                    (flags, changed || removed)
                });

        if changed {
            let prev_path = self.path();
            let next_path = prev_path.with_file_name(self.format_file_name(flags)?);

            fs::rename(prev_path, &next_path)?;
            self.path = next_path;
        }

        Ok(())
    }

    pub fn copy(&self, mdir: &Maildir) -> Result<()> {
        if Some(mdir.cur()) == self.path().parent() {
            return Ok(());
        }

        let file_name = self.file_name()?;
        fs::copy(self.path(), mdir.cur().join(file_name))?;

        Ok(())
    }

    pub fn r#move(&self, mdir: &Maildir) -> Result<()> {
        if Some(mdir.cur()) == self.path().parent() {
            return Ok(());
        }

        let file_name = self.file_name()?;
        fs::rename(self.path(), mdir.cur().join(file_name))?;

        Ok(())
    }

    pub fn remove(&self) -> Result<()> {
        fs::remove_file(self.path())?;
        Ok(())
    }

    fn format_file_name(&self, flags: HashSet<Flag>) -> Result<String> {
        Ok(format_file_name(self.info_separator, self.id()?, flags))
    }
}

fn format_file_name(sep: &'static str, id: impl AsRef<str>, flags: HashSet<Flag>) -> String {
    let id = id.as_ref();

    let mut flags: Vec<&str> = flags.iter().map(AsRef::as_ref).collect();
    flags.sort();

    format!("{id}{sep}2,{flags}", flags = flags.join(""))
}
