//! Mutate only current validated state while retaining the stable owner lock.

use std::{fs, path::Path};

use magistr_workflow_contracts::{ReadError, ReadSource, WriteError};
use serde::{Serialize, de::DeserializeOwned};

use super::{Committed, TransactionError, TransactionTarget, write_locked};
use crate::infra::read_json;

/// The caller installs the committed state and publishes only after this returns.
/// Missing durable state starts from `empty`, never from an unobserved memory copy.
pub fn transact<T: Clone + Serialize + DeserializeOwned, R, E>(
    target: TransactionTarget<'_, T>,
    empty: impl FnOnce() -> T,
    validate: impl Fn(&T) -> Result<(), ReadError>,
    apply: impl FnOnce(&mut T) -> Result<R, E>,
) -> Result<Committed<T, R>, TransactionError<E>> {
    match target {
        TransactionTarget::Memory(memory) => {
            let mut state = memory.clone();
            validate(&state).map_err(read_failure)?;
            let value = apply(&mut state).map_err(TransactionError::Rejected)?;
            validate(&state).map_err(read_failure)?;
            Ok(Committed {
                state,
                value,
                source: ReadSource::Memory,
            })
        }
        TransactionTarget::File { path, lock_path } => {
            let _lock = lock(lock_path).map_err(TransactionError::Store)?;
            let observed = read_json(path).map_err(read_failure)?;
            let mut state = observed.value.unwrap_or_else(empty);
            validate(&state).map_err(read_failure)?;
            let value = apply(&mut state).map_err(TransactionError::Rejected)?;
            validate(&state).map_err(read_failure)?;
            let stamp = write_locked(path, &state).map_err(TransactionError::Store)?;
            Ok(Committed {
                state,
                value,
                source: ReadSource::File(stamp),
            })
        }
    }
}

fn read_failure<E>(error: ReadError) -> TransactionError<E> {
    TransactionError::Store(WriteError::read(error))
}

fn lock(path: &Path) -> Result<fs::File, WriteError> {
    let open = || -> std::io::Result<fs::File> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let file = fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .open(path)?;
        if !file.metadata()?.is_file() {
            return Err(std::io::Error::other(
                "The owner lock is not a regular file.",
            ));
        }
        file.lock()?;
        Ok(file)
    };
    open().map_err(|error| {
        tracing::warn!(path = %path.display(), %error, "Workflow mutation lock could not be acquired");
        WriteError::before_commit("Your changes could not be saved. Restore access and try again.")
    })
}

#[cfg(test)]
#[path = "apply.tests.rs"]
mod tests;
