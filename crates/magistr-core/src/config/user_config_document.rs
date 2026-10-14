//! Validated, revision-checked edits to personal configuration. Readers and
//! writers share this document so comments and unrelated preferences survive.

use std::fs::{self, File, TryLockError};
use std::io;
use std::path::{Path, PathBuf};

use toml_edit::{DocumentMut, Item, Table, Value};

use super::{UserConfig, UserConfigError};

/// An exact observation of personal settings and the bytes that established it.
#[derive(Debug)]
pub struct UserConfigDocument {
    path: PathBuf,
    original: Option<String>,
    config: UserConfig,
    document: DocumentMut,
}

impl UserConfigDocument {
    /// Read without creating a directory or file. Only a missing file uses defaults.
    pub fn load(path: &Path) -> Result<Self, UserConfigError> {
        Self::parse(path, read_original(path)?)
    }

    fn parse(path: &Path, original: Option<String>) -> Result<Self, UserConfigError> {
        let text = original.as_deref().unwrap_or_default();
        let config = toml::from_str(text)
            .map_err(|error| UserConfigError::Parse(path.to_owned(), error.to_string()))?;
        let document = text
            .parse::<DocumentMut>()
            .map_err(|error| UserConfigError::Parse(path.to_owned(), error.to_string()))?;
        Ok(Self {
            path: path.to_owned(),
            original,
            config,
            document,
        })
    }

    #[must_use]
    pub fn config(&self) -> &UserConfig {
        &self.config
    }

    #[must_use]
    pub fn is_present(&self) -> bool {
        self.original.is_some()
    }

    /// Save section/key changes only if the observed bytes remain current.
    ///
    /// Every personal-settings writer uses the same stable sibling lock. A
    /// crash releases its OS lock; the file remains so later writers lock the
    /// same object. Both old and new documents must satisfy the current schema.
    pub fn save(&self, changes: &[(&str, &str, Value)]) -> Result<Self, UserConfigError> {
        let parent = self
            .path
            .parent()
            .filter(|path| !path.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."));
        fs::create_dir_all(parent)
            .map_err(|error| UserConfigError::Io(parent.to_owned(), error))?;
        let lock_path = lock_path(&self.path);
        let lock = File::options()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&lock_path)
            .map_err(|error| UserConfigError::Io(lock_path.clone(), error))?;
        match lock.try_lock() {
            Ok(()) => {}
            Err(TryLockError::WouldBlock) => return Err(UserConfigError::Busy(self.path.clone())),
            Err(TryLockError::Error(error)) => return Err(UserConfigError::Io(lock_path, error)),
        }
        if read_original(&self.path)? != self.original {
            return Err(UserConfigError::Conflict(self.path.clone()));
        }
        let mut document = self.document.clone();
        for (section, key, setting) in changes {
            let table = document
                .entry(section)
                .or_insert(Item::Table(Table::new()))
                .as_table_like_mut()
                .ok_or_else(|| {
                    UserConfigError::Parse(
                        self.path.clone(),
                        format!("The {section} settings must be a table."),
                    )
                })?;
            let mut setting = setting.clone();
            if let Some(previous) = table.get(key).and_then(Item::as_value) {
                *setting.decor_mut() = previous.decor().clone();
            }
            table.insert(key, Item::Value(setting));
        }
        let updated = document.to_string();
        let saved = Self::parse(&self.path, Some(updated.clone()))?;
        crate::durable_write::durable_write(&self.path, updated.as_bytes())
            .map_err(|error| UserConfigError::Io(self.path.clone(), error))?;
        Ok(saved)
    }
}

fn lock_path(path: &Path) -> PathBuf {
    let mut name = path.as_os_str().to_os_string();
    name.push(".lock");
    PathBuf::from(name)
}

fn read_original(path: &Path) -> Result<Option<String>, UserConfigError> {
    match fs::read_to_string(path) {
        Ok(text) => Ok(Some(text)),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(UserConfigError::Io(path.to_owned(), error)),
    }
}

#[cfg(test)]
#[path = "user_config_document.tests.rs"]
mod tests;
