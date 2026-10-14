//! Canonical file locations and strict persistence for workflow-owned documents.
//! Mutations share the transaction seam; an unreadable store never becomes absence.

use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Serialize, de::DeserializeOwned};

use super::read_json;
use super::transaction::{TransactionError, TransactionTarget, transact};
use magistr_workflow_contracts::{Observed, ReadError, ReadSource, WriteError};

const PERSIST_VAR: &str = "THINK_AND_SHIP_PERSIST";
const DATA_DIR_VAR: &str = "THINK_AND_SHIP_DATA_DIR";

/// Which tool family's subdirectory to read or write under.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Domain {
    Think,
    Ship,
    Roadmap,
    Signal,
    /// Adopted W3C Trace Context (SEP-414). Not a tool family — the one
    /// partition written by the MCP server and read by the `trace export`
    /// CLI in a later process.
    Otel,
    /// Per-tool invocation counts ([`crate::usage`]). Not a tool family, and
    /// deliberately its OWN partition rather than a field on any family's
    /// state: the counter must be readable by the `calls` CLI without loading
    /// a trace, and it must be visibly out of reach of the telemetry
    /// extractor, which reads no partition but its own.
    Usage,
}

impl Domain {
    fn dir_name(self) -> &'static str {
        match self {
            Self::Think => "think",
            Self::Ship => "ship",
            Self::Roadmap => "roadmap",
            Self::Signal => "signal",
            Self::Otel => "otel",
            Self::Usage => "usage",
        }
    }
}

#[derive(Debug, Clone)]
pub struct PersistenceConfig {
    pub enabled: bool,
    pub data_dir: PathBuf,
}

impl PersistenceConfig {
    /// Resolve from environment. Off by default; opt in with
    /// `THINK_AND_SHIP_PERSIST=true` (or `=1`).
    ///
    /// A blank `THINK_AND_SHIP_DATA_DIR` falls through to the platform chain
    /// rather than resolving to the empty path. Set-but-empty is how a shell
    /// exports a variable it failed to compute, and taking it literally would
    /// point one family at a relative `think/sessions` under the working
    /// directory while every family that guarded against it kept using the
    /// real data root — the same process, its records split across two roots.
    pub fn from_env() -> Self {
        let enabled = env::var(PERSIST_VAR)
            .map(|v| v == "true" || v == "1")
            .unwrap_or(false);
        let data_dir = env::var(DATA_DIR_VAR)
            .ok()
            .filter(|v| !v.trim().is_empty())
            .map(PathBuf::from)
            .unwrap_or_else(default_data_dir);
        Self { enabled, data_dir }
    }

    pub fn with_data_dir(mut self, dir: PathBuf) -> Self {
        self.data_dir = dir;
        self
    }

    pub fn enabled(mut self, on: bool) -> Self {
        self.enabled = on;
        self
    }
}

/// The one data-root fallback chain, shared by every family's config so a
/// process without `HOME` cannot scatter the families across different roots.
pub(crate) fn default_data_dir() -> PathBuf {
    default_data_dir_from(|key| env::var(key).ok())
}

/// Resolve the platform data root through an injected lookup so tests can
/// drive every arm — including the Windows ones — on any OS without touching
/// process environment. Chain: `XDG_DATA_HOME` → `HOME/.local/share` →
/// `%APPDATA%` → `%USERPROFILE%\AppData\Roaming` → the OS temp dir. A set
/// An explicit home takes precedence over the platform application-data path.
pub(crate) fn default_data_dir_from(get: impl Fn(&str) -> Option<String>) -> PathBuf {
    let non_empty = |key: &str| get(key).filter(|v| !v.trim().is_empty());
    if let Some(xdg) = non_empty("XDG_DATA_HOME") {
        return PathBuf::from(xdg).join("think-and-ship");
    }
    if let Some(home) = non_empty("HOME") {
        return PathBuf::from(home)
            .join(".local")
            .join("share")
            .join("think-and-ship");
    }
    if let Some(appdata) = non_empty("APPDATA") {
        return PathBuf::from(appdata).join("think-and-ship");
    }
    if let Some(profile) = non_empty("USERPROFILE") {
        return PathBuf::from(profile)
            .join("AppData")
            .join("Roaming")
            .join("think-and-ship");
    }
    // No home of any shape (some CI). Persistence still functions; the data
    // is just ephemeral, and the path is valid on both platforms — a literal
    // `/tmp` is not a real place on Windows.
    env::temp_dir().join("think-and-ship")
}

/// Per-domain persistence handle. Created with `Persistence::new`.
#[derive(Debug, Clone)]
pub struct Persistence {
    enabled: bool,
    sessions_dir: PathBuf,
}

impl Persistence {
    pub fn new(cfg: &PersistenceConfig, domain: Domain) -> Self {
        let sessions_dir = cfg.data_dir.join(domain.dir_name()).join("sessions");
        if cfg.enabled
            && let Err(e) = fs::create_dir_all(&sessions_dir)
        {
            tracing::warn!(
                "think-and-ship: could not create data dir {}: {e}",
                sessions_dir.display()
            );
        }
        Self {
            enabled: cfg.enabled,
            sessions_dir,
        }
    }

    pub fn enabled(&self) -> bool {
        self.enabled
    }

    pub fn sessions_dir(&self) -> &Path {
        &self.sessions_dir
    }

    /// The store file backing one session. `pub` so a caller that must operate
    /// on the file itself — `roadmap prune`, which backs it up before writing —
    /// doesn't have to reconstruct the path and risk disagreeing with this.
    pub fn path_for(&self, session_id: &str) -> PathBuf {
        self.sessions_dir.join(format!("{session_id}.json"))
    }

    /// Replace one complete document under its owner lock.
    /// Existing bytes must still decode before a replacement is admitted.
    pub fn save<T: Clone + Serialize + DeserializeOwned>(
        &self,
        session_id: &str,
        state: &T,
    ) -> Result<Observed<T>, WriteError> {
        self.save_merging(session_id, state, |current, _disk| current.clone())
    }

    /// Fold a mutation delta into current saved data and return exactly what committed.
    pub fn save_merging<T, F>(
        &self,
        session_id: &str,
        state: &T,
        merge: F,
    ) -> Result<Observed<T>, WriteError>
    where
        T: Clone + Serialize + DeserializeOwned,
        F: FnOnce(&T, T) -> T,
    {
        if !self.enabled {
            return Ok(Observed {
                value: state.clone(),
                source: ReadSource::Memory,
            });
        }
        locked_merge_write(&self.path_for(session_id), state, merge)
    }

    /// Read and deserialize the persisted state for `session_id`. Returns
    /// `Ok(None)` when persistence is off or the file doesn't exist.
    pub fn load<T: DeserializeOwned>(&self, session_id: &str) -> Result<Option<T>, ReadError> {
        self.load_stamped(session_id).map(|observed| observed.value)
    }

    /// Read validated records and the source of those exact bytes.
    /// Absence and disabled persistence remain distinct from a stored empty value.
    pub fn load_stamped<T: DeserializeOwned>(
        &self,
        session_id: &str,
    ) -> Result<Observed<Option<T>>, ReadError> {
        if !self.enabled {
            return Ok(Observed {
                value: None,
                source: ReadSource::Memory,
            });
        }
        read_json(&self.path_for(session_id))
    }
}

/// Merge a delta only after the current document has been read successfully.
/// This is for complete unversioned ancillary records; workflow families use
/// `transact` directly with their identity and schema validators.
pub(crate) fn locked_merge_write<T, F>(
    path: &Path,
    state: &T,
    merge: F,
) -> Result<Observed<T>, WriteError>
where
    T: Clone + Serialize + DeserializeOwned,
    F: FnOnce(&T, T) -> T,
{
    let missing = std::cell::Cell::new(false);
    let committed = transact(
        TransactionTarget::File {
            path,
            lock_path: &path.with_extension("json.lock"),
        },
        || {
            missing.set(true);
            state.clone()
        },
        |_| Ok(()),
        |candidate| {
            if !missing.get() {
                *candidate = merge(state, candidate.clone());
            }
            Ok::<_, std::convert::Infallible>(())
        },
    )
    .map_err(|error| match error {
        TransactionError::Store(error) => error,
        TransactionError::Rejected(never) => match never {},
    })?;
    Ok(Observed {
        value: committed.state,
        source: committed.source,
    })
}

#[cfg(test)]
#[path = "persistence.tests.rs"]
mod tests;
