use std::{io, path::PathBuf, result, str::Utf8Error, time::SystemTimeError};

use thiserror::Error;

pub type Result<T> = result::Result<T, Error>;

/// Crate-local error type.
#[derive(Debug, Error)]
pub enum Error {
    #[error("cannot read Maildir at {0}")]
    ReadMaildirError(PathBuf),
    #[error("cannot find newly created maildir entry at {0}")]
    FindNewMaildirEntryError(PathBuf),
    #[error("cannot get maildir entry file name at {0}")]
    GetMaildirEntryFileNameError(PathBuf),
    #[error("cannot get maildir entry file name at {0}")]
    GetInvalidMaildirEntryFileNameError(#[source] Utf8Error, PathBuf),
    #[error("cannot find maildir matching name {0}")]
    GetMaildirByNameNotFoundError(String),
    #[error("cannot find valid maildir matching name {1} at {0}")]
    GetMaildirByNameInvalidError(PathBuf, String),
    #[error("cannot get maildir name at {0}")]
    GetMaildirNameError(PathBuf),
    #[error("cannot remove maildir matching name {0}")]
    RemoveMaildirByNameNotFoundError(String),
    #[error("cannot find maildir entry matching {0}")]
    GetMaildirEntryNotFoundError(String),

    #[error("cannot find email {0}")]
    FindEmailError(String),
    #[error("cannot copy email to the same path {0}")]
    CopyEmailSamePathError(PathBuf),
    #[error("cannot get parent of")]
    NoParentError(PathBuf),
    #[error("invalid id {0}")]
    InvalidIdError(String),
    #[error("invalid folder {0}")]
    InvalidFolderError(String),
    #[error("invalid flag {0}")]
    InvalidFlagError(char),
    #[error("{0} already exists")]
    AlreadyExistsError(PathBuf),
    #[error(transparent)]
    Utf8Error(#[from] Utf8Error),
    #[error(transparent)]
    IoError(#[from] io::Error),
    #[error(transparent)]
    SystemTimeError(#[from] SystemTimeError),
}
