use crate::contracts::{ErrorKind, WorkspaceError};
use serde::Serialize;

/// The retained wire estimator uses JavaScript string length, hence UTF-16 units.
pub(crate) fn tokens<R: Serialize>(row: &R) -> Result<u64, WorkspaceError> {
    let text = serde_json::to_string(row).map_err(|error| {
        WorkspaceError::new(
            ErrorKind::PatternInvalid,
            format!("cannot measure query row: {error}"),
        )
    })?;
    Ok((text.encode_utf16().count() as u64).div_ceil(4))
}

#[cfg(test)]
#[path = "tokens.tests.rs"]
mod tests;
