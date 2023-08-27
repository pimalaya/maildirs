use std::{
    fmt::{self, Display, Formatter},
    io,
    path::PathBuf,
    str::Utf8Error,
    time::SystemTimeError,
};

/// Crate-local error type.
#[non_exhaustive]
#[derive(Debug)]
pub enum Error {
    /// An email message could not be found with the given id.
    FindEmailError(String),
    /// The supplied path was an invalid filename.
    InvalidFilenameError(String),
    /// Tried to copy an email to the same path.
    CopyEmailSamePathError(PathBuf),
    /// Cannot determine the parent of the supplied path.
    NoParentError(PathBuf),
    /// The supplied id was invalid.
    InvalidIdError(String),
    /// The supplied folder name was invalid.
    InvalidFolderError(String),
    /// The supplied flag was invalid.
    InvalidFlagError(char),
    /// The file already exists
    AlreadyExistsError(PathBuf),
    /// UTF-8 error
    Utf8Error(Utf8Error),
    /// IO error
    IoError(io::Error),
    /// System time error
    SystemTimeError(SystemTimeError),
}

impl Display for Error {
    fn fmt(&self, f: &mut Formatter) -> Result<(), fmt::Error> {
        use Error::*;
        match self {
            FindEmailError(id) => write!(f, "cannot find email {id}"),
            InvalidFilenameError(filename) => {
                write!(f, "cannot get email from invalid file {filename}")
            }
            CopyEmailSamePathError(path) => {
                write!(f, "cannot copy email to the same path {}", path.display())
            }
            NoParentError(path) => write!(f, "cannot get parent of {}", path.display()),
            InvalidIdError(id) => write!(f, "invalid id {id}"),
            InvalidFolderError(folder) => write!(f, "invalid folder {folder}"),
            InvalidFlagError(flag) => write!(f, "invalid flag {flag}"),
            AlreadyExistsError(path) => write!(f, "{} already exists", path.display()),
            Utf8Error(err) => write!(f, "{err}"),
            IoError(err) => write!(f, "{err}"),
            SystemTimeError(err) => write!(f, "{err}"),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Error::FindEmailError(_) => None,
            Error::InvalidFilenameError(_) => None,
            Error::CopyEmailSamePathError(_) => None,
            Error::NoParentError(_) => None,
            Error::InvalidIdError(_) => None,
            Error::InvalidFolderError(_) => None,
            Error::InvalidFlagError(_) => None,
            Error::AlreadyExistsError(_) => None,
            Error::Utf8Error(err) => Some(err),
            Error::IoError(err) => Some(err),
            Error::SystemTimeError(err) => Some(err),
        }
    }
}

impl From<Utf8Error> for Error {
    fn from(err: Utf8Error) -> Self {
        Error::Utf8Error(err)
    }
}

impl From<io::Error> for Error {
    fn from(err: io::Error) -> Self {
        Error::IoError(err)
    }
}

impl From<SystemTimeError> for Error {
    fn from(err: SystemTimeError) -> Self {
        Error::SystemTimeError(err)
    }
}
