use crate::Error;

pub(crate) fn validate_id(id: String) -> Result<String, Error> {
    if id.contains('/') {
        Err(Error::InvalidIdError(id))
    } else {
        Ok(id)
    }
}

pub(crate) fn validate_folder(folder: &str) -> Result<(), Error> {
    if folder.contains('/') {
        Err(Error::InvalidFolderError(folder.to_string()))
    } else {
        Ok(())
    }
}
