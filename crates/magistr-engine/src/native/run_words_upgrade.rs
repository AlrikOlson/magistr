//! The one-time move of saved work to the run vocabulary.
//!
//! The run store moves from `ship/` to `run/` (its check results from
//! `receipts/` to `check_results/`), and each project's plan and decision
//! documents rewrite their run references (`task:` to `step:`, `action:` to
//! `progress:`). Each family's own codec converts its documents; this module
//! only owns where they live and the order of the move. Admission finishes the
//! move before any engine opens; readers refuse while it is unfinished.

use super::NativeConfig;
use magistr_workflow_contracts::{ReadError, ReadErrorKind};
use std::{
    fs,
    io::{self, Write},
    path::{Path, PathBuf},
};

/// Converts saved work to the run vocabulary exactly once.
pub struct RunWordsUpgrade;

/// The store directory the run family used before the vocabulary cutover.
const OLD_RUN_DIR: &str = "ship";
const RUN_DIR: &str = "run";
const STAGING_DIR: &str = "run.upgrading";
const LOCK: &str = "run-words-upgrade.lock";

impl RunWordsUpgrade {
    /// Refuse reads while saved work still waits for this upgrade: the run
    /// store has not moved yet, or this project's decision document is still
    /// at the version that spelled run references the old way. Cheap: one
    /// metadata lookup and the first bytes of one document.
    pub(super) fn ready(config: &NativeConfig, project_id: &str) -> Result<(), ReadError> {
        if !config.persist {
            return Ok(());
        }
        let root = config.data_dir();
        let decisions = &project_documents(root, project_id)[1].0;
        if root.join(OLD_RUN_DIR).exists() || decision_version(decisions) == Some(2) {
            return Err(ReadError::new(
                ReadErrorKind::Interrupted,
                "Saved work is waiting for a one-time upgrade. Start Magistr to finish it.",
            ));
        }
        Ok(())
    }

    /// Finish the upgrade for the store and for `project_id`. The caller holds
    /// the project's owner lease; the store-wide move also holds its own lock,
    /// so two projects admitted at once never move the run store twice.
    ///
    /// # Errors
    /// Any I/O failure, or a saved record its family codec refuses. Originals
    /// stay where they are until their converted copy is durable.
    pub(super) fn apply(config: &NativeConfig, project_id: &str) -> io::Result<()> {
        if !config.persist {
            return Ok(());
        }
        let root = config.data_dir();
        fs::create_dir_all(root)?;
        let lock = fs::File::options()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(root.join(LOCK))?;
        lock.lock()?;
        move_run_store(root, project_id)?;
        // Converting a current document changes nothing, so this runs on every
        // admission and needs no marker of its own.
        convert_project(config, project_id)
    }
}

/// The `schema_version` a decision document opens with, read from its first
/// bytes; the writer always serializes it first.
fn decision_version(path: &Path) -> Option<u64> {
    use std::io::Read;
    let mut head = [0u8; 64];
    let read = fs::File::open(path).ok()?.read(&mut head).ok()?;
    let text = std::str::from_utf8(&head[..read]).ok()?;
    let rest = text.split_once("\"schema_version\":")?.1.trim_start();
    let digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
    digits.parse().ok()
}

#[derive(Clone, Copy)]
enum Family {
    Plan,
    Decision,
}

fn project_documents(root: &Path, project_id: &str) -> [(PathBuf, Family); 2] {
    [
        (
            root.join("roadmap/sessions")
                .join(format!("{project_id}.json")),
            Family::Plan,
        ),
        (
            root.join("think/sessions")
                .join(format!("{project_id}.json")),
            Family::Decision,
        ),
    ]
}

/// Convert the whole run store into a staging directory, publish it as
/// `run/` with one rename, then keep the original as a backup. An
/// interrupted move leaves `ship/` untouched and is rebuilt from scratch.
fn move_run_store(root: &Path, project_id: &str) -> io::Result<()> {
    let old = root.join(OLD_RUN_DIR);
    if !old.exists() {
        return Ok(());
    }
    // An older owner still running for another project would keep writing
    // into the old directory after the move and its work would split.
    let live = live_owners(root, project_id)?;
    if !live.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::WouldBlock,
            format!(
                "Saved work needs a one-time upgrade, but Magistr is still running for {}. Close those sessions, then start Magistr again; nothing was changed.",
                live.join(", ")
            ),
        ));
    }
    let staging = root.join(STAGING_DIR);
    if staging.exists() {
        fs::remove_dir_all(&staging)?;
    }
    convert_tree(&old, &staging, Path::new(""))?;
    let current = root.join(RUN_DIR);
    if current.exists() {
        if holds_records(&current)? {
            return Err(io::Error::other(format!(
                "Saved run records exist in both {} and {}. Both are intact; keep the copy you want in {} and retry.",
                old.display(),
                current.display(),
                old.display()
            )));
        }
        fs::remove_dir_all(&current)?;
    }
    fs::rename(&staging, &current)?;
    sync_directory(root)?;
    let backups = root.join("backups");
    fs::create_dir_all(&backups)?;
    let stamp = chrono::Utc::now().format("%Y%m%dT%H%M%SZ");
    fs::rename(&old, backups.join(format!("ship-before-run-words-{stamp}")))?;
    sync_directory(root)?;
    sync_directory(&backups)
}

/// Projects other than `project_id` whose owner currently holds its store lock.
fn live_owners(root: &Path, project_id: &str) -> io::Result<Vec<String>> {
    let sessions = root.join("roadmap/sessions");
    let entries = match fs::read_dir(&sessions) {
        Ok(entries) => entries,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(error),
    };
    let mut live = std::collections::BTreeSet::new();
    for entry in entries {
        let name = entry?.file_name().to_string_lossy().into_owned();
        let Some(project) = name
            .strip_suffix("-owner.lock")
            .and_then(|stem| stem.rsplit_once('.'))
            .map(|(project, _role)| project.to_owned())
        else {
            continue;
        };
        if project == project_id {
            continue;
        }
        let file = fs::File::options()
            .read(true)
            .write(true)
            .open(sessions.join(&name))?;
        if matches!(file.try_lock(), Err(fs::TryLockError::WouldBlock)) {
            live.insert(project);
        }
    }
    Ok(live.into_iter().collect())
}

/// Copy `source` into `target`, renaming `receipts/` directories to
/// `check_results/`, converting run documents and check results through the
/// run family's codec, and leaving lock files behind.
fn convert_tree(source: &Path, target: &Path, relative: &Path) -> io::Result<()> {
    fs::create_dir_all(target)?;
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let kind = entry.file_type()?;
        let name = entry.file_name();
        let path = entry.path();
        if kind.is_symlink() {
            return Err(io::Error::other(format!(
                "{} must not be a symbolic link during upgrade",
                path.display()
            )));
        }
        if kind.is_dir() {
            let renamed = if name == "receipts" {
                "check_results".into()
            } else {
                name.clone()
            };
            convert_tree(&path, &target.join(&renamed), &relative.join(&name))?;
            continue;
        }
        let name = name.to_string_lossy();
        if name.ends_with(".lock") {
            continue;
        }
        let bytes = fs::read(&path)?;
        let converted = if !name.ends_with(".json") {
            bytes
        } else if relative
            .components()
            .any(|part| part.as_os_str() == "receipts")
        {
            magistr_workflow::run::persistence::Persistence::upgrade_check_result(&bytes)
                .map_err(|error| invalid(&path, error))?
        } else if relative.ends_with("sessions") {
            let project_id = name.trim_end_matches(".json");
            magistr_workflow::run::persistence::Persistence::upgrade_document(&bytes, project_id)
                .map_err(|error| invalid(&path, error))?
        } else {
            bytes
        };
        publish(&target.join(name.as_ref()), &converted, false)?;
    }
    sync_directory(target)
}

/// Whether `directory` holds any regular file. Opening the store before the
/// upgrade may create empty directories, never records.
fn holds_records(directory: &Path) -> io::Result<bool> {
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        let kind = entry.file_type()?;
        if kind.is_dir() {
            if holds_records(&entry.path())? {
                return Ok(true);
            }
        } else {
            return Ok(true);
        }
    }
    Ok(false)
}

/// Rewrite this project's plan and decision documents through their own
/// codecs, each under its document lock, keeping the original beside the
/// backups first.
fn convert_project(config: &NativeConfig, project_id: &str) -> io::Result<()> {
    let root = config.data_dir();
    for (path, family) in project_documents(root, project_id) {
        let Ok(bytes) = fs::read(&path) else {
            continue;
        };
        let lock = fs::File::options()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(path.with_extension("json.lock"))?;
        lock.lock()?;
        let converted = match family {
            Family::Plan => {
                magistr_workflow::roadmap::RoadmapEngine::upgrade_document(&bytes, project_id)
                    .map_err(|error| invalid(&path, error))?
            }
            Family::Decision => {
                let mut histories = std::collections::BTreeMap::new();
                histories.insert(project_id.to_owned(), bytes.clone());
                magistr_workflow::think::persistence::Persistence::for_project(
                    &config.persistence_config(),
                    project_id,
                )
                .upgrade_documents(&histories)
                .map_err(|error| invalid(&path, error))?
            }
        };
        if converted == bytes {
            continue;
        }
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("record.json");
        let family_dir = path
            .parent()
            .and_then(Path::parent)
            .and_then(Path::file_name)
            .and_then(|name| name.to_str())
            .unwrap_or("workflow");
        let backup = root
            .join("backups/before-run-words")
            .join(family_dir)
            .join(name);
        if !backup.exists() {
            publish(&backup, &bytes, false)?;
        }
        publish(&path, &converted, true)?;
    }
    Ok(())
}

fn invalid(path: &Path, error: impl std::fmt::Display) -> io::Error {
    io::Error::other(format!(
        "{} could not be upgraded and is unchanged: {error}",
        path.display()
    ))
}

fn publish(path: &Path, bytes: &[u8], replace: bool) -> io::Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| io::Error::other("A saved record has no directory."))?;
    fs::create_dir_all(parent)?;
    let mut staged = tempfile::NamedTempFile::new_in(parent)?;
    staged.write_all(bytes)?;
    staged.as_file().sync_all()?;
    if replace {
        staged.persist(path).map_err(|e| e.error)?;
    } else {
        staged.persist_noclobber(path).map_err(|e| e.error)?;
    }
    sync_directory(parent)
}

/// Make a new directory entry durable. Windows cannot open a directory as a
/// file and commits the entry with the rename itself.
fn sync_directory(directory: &Path) -> io::Result<()> {
    #[cfg(unix)]
    fs::File::open(directory)?.sync_all()?;
    #[cfg(not(unix))]
    let _ = directory;
    Ok(())
}

#[cfg(test)]
#[path = "run_words_upgrade.tests.rs"]
mod tests;
