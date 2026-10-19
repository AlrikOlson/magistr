//! The settings writer releases ownership explicitly, including while another
//! thread's freshly forked child briefly holds an inherited descriptor.
use super::UserConfigError;
use std::{
    fs::{File, TryLockError},
    path::Path,
};

pub(super) struct UserConfigLock(File);

impl UserConfigLock {
    pub(super) fn acquire(lock_path: &Path, settings: &Path) -> Result<Self, UserConfigError> {
        let file = File::options()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(lock_path)
            .map_err(|error| UserConfigError::Io(lock_path.to_owned(), error))?;
        match file.try_lock() {
            Ok(()) => Ok(Self(file)),
            Err(TryLockError::WouldBlock) => Err(UserConfigError::Busy(settings.to_owned())),
            Err(TryLockError::Error(error)) => {
                Err(UserConfigError::Io(lock_path.to_owned(), error))
            }
        }
    }
}

impl Drop for UserConfigLock {
    fn drop(&mut self) {
        let _ = self.0.unlock();
    }
}

#[cfg(test)]
#[path = "user_config_lock.tests.rs"]
mod tests;
