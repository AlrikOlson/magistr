//! Validated JSON observations bind decoded records to the file descriptor read.

use std::{
    fs::{self, File},
    io::{self, Read},
    path::Path,
};

use magistr_workflow_contracts::{Observed, ReadError, ReadErrorKind, ReadSource};
use serde::de::DeserializeOwned;

use super::store_stamp;

pub(crate) fn read_json<T: DeserializeOwned>(
    path: &Path,
) -> Result<Observed<Option<T>>, ReadError> {
    let mut file = match store_stamp::open_regular(path) {
        Ok(file) => file,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return Ok(Observed {
                value: None,
                source: ReadSource::Missing,
            });
        }
        Err(error) => return Err(io_error(path, error)),
    };
    let before = file.metadata().map_err(|error| io_error(path, error))?;
    read_open_json(&mut file, before).map_err(|error| {
        tracing::warn!(path = %path.display(), %error, "Saved work could not be observed");
        error
    })
}

fn read_open_json<T: DeserializeOwned>(
    file: &mut File,
    before: fs::Metadata,
) -> Result<Observed<Option<T>>, ReadError> {
    store_stamp::from_metadata(&before).map_err(metadata_error)?;
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes).map_err(metadata_error)?;
    let after = file.metadata().map_err(metadata_error)?;
    if !store_stamp::unchanged(&before, &after).map_err(metadata_error)?
        || after.len() != bytes.len() as u64
    {
        return Err(ReadError::new(
            ReadErrorKind::Interrupted,
            "The saved work changed while it was being read. Try again.",
        ));
    }
    let value = serde_json::from_slice(&bytes).map_err(|error| {
        tracing::warn!(%error, "Saved work contains invalid JSON");
        ReadError::new(
            ReadErrorKind::Decode,
            "The saved work could not be understood.",
        )
    })?;
    let stamp = store_stamp::from_metadata(&after).map_err(metadata_error)?;
    Ok(Observed {
        value: Some(value),
        source: ReadSource::File(stamp),
    })
}

fn io_error(path: &Path, error: io::Error) -> ReadError {
    tracing::warn!(path = %path.display(), %error, "Saved work could not be opened");
    metadata_error(error)
}

fn metadata_error(error: io::Error) -> ReadError {
    tracing::warn!(%error, "Saved work could not be read");
    ReadError::new(
        ReadErrorKind::Io,
        "The saved work could not be read. Restore access and try again.",
    )
}

#[cfg(test)]
#[path = "store_read.tests.rs"]
mod tests;
