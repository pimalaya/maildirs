use crate::Error;

/// Represents a maildir flag.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Flag {
    Passed,
    Replied,
    Seen,
    Trashed,
    Draft,
    Flagged,
}

impl AsRef<str> for Flag {
    fn as_ref(&self) -> &str {
        match self {
            Flag::Passed => "P",
            Flag::Replied => "R",
            Flag::Seen => "S",
            Flag::Trashed => "T",
            Flag::Draft => "D",
            Flag::Flagged => "F",
        }
    }
}

impl TryFrom<char> for Flag {
    type Error = Error;

    fn try_from(s: char) -> Result<Self, Error> {
        match s {
            'P' => Ok(Flag::Passed),
            'R' => Ok(Flag::Replied),
            'S' => Ok(Flag::Seen),
            'T' => Ok(Flag::Trashed),
            'D' => Ok(Flag::Draft),
            'F' => Ok(Flag::Flagged),
            _ => Err(Error::InvalidFlagError(s)),
        }
    }
}
