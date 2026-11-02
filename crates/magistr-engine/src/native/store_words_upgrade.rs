//! The one-time move of saved work to the glossary's vocabulary.
//!
//! Each family whose words changed moves its store directory once, in a fixed
//! order: the run store from `ship/` to `run/` (its check results from
//! `receipts/` to `check_results/`), then the plan store from `roadmap/` to
//! `plan/`, then the decision store from `think/` to `decisions/`. Each
//! project's plan, decision and signal documents then convert through their
//! own codecs, which decide what is old from the document's version, never
//! from its references: after the plan words `task:` names a plan task, while
//! before the run words it named a run step. This module
//! only owns where saved work lives and the order of the moves. Admission
//! finishes them before any engine opens; readers refuse while one is
//! unfinished.

use super::NativeConfig;
use magistr_workflow_contracts::{ReadError, ReadErrorKind};
use std::{
    fs,
    io::{self, Write},
    path::{Path, PathBuf},
};

/// Converts saved work to the glossary's vocabulary exactly once.
pub struct StoreWordsUpgrade;

const LOCK: &str = "store-words-upgrade.lock";

/// The current schema versions of the per-project documents whose saved
/// references changed words. Older documents are refused until converted.
const DECISION_VERSION: u64 = 5;
const RUN_VERSION: u64 = 5;
const PLAN_VERSION: u64 = 3;
const SIGNALS_VERSION: u64 = 3;

/// One family store directory that moves to its new name.
#[derive(Clone, Copy)]
struct Move {
    old: &'static str,
    new: &'static str,
    words: &'static str,
    codec: Codec,
}

/// The order is the glossary's: the run store has finished before the plan
/// store converts its references, and the plan store before the decision
/// store.
const MOVES: [Move; 3] = [
    Move {
        old: "ship",
        new: "run",
        words: "run-words",
        codec: Codec::Run,
    },
    Move {
        old: "roadmap",
        new: "plan",
        words: "plan-words",
        codec: Codec::Plan,
    },
    Move {
        old: "think",
        new: "decisions",
        words: "decision-words",
        codec: Codec::Decision,
    },
];

/// Where older owners hold their project locks: before the plan words in the
/// old plan directory, and until the decision words in the plan directory.
/// Current owners lock under [`super::owner_lease::OWNER_LEASES`], so every
/// held lock in these is an older owner's.
const OLDER_OWNER_LOCKS: [&str; 2] = ["roadmap/sessions", "plan/sessions"];

#[derive(Clone, Copy)]
enum Codec {
    Run,
    Plan,
    Decision,
}

impl StoreWordsUpgrade {
    /// Refuse reads while saved work still waits for this upgrade: a family
    /// store has not moved yet, or one of this project's documents is still
    /// at a version that spelled its references the old way. Cheap: metadata
    /// lookups and the first bytes of this project's documents.
    pub(super) fn ready(config: &NativeConfig, project_id: &str) -> Result<(), ReadError> {
        if !config.persist {
            return Ok(());
        }
        let root = config.data_dir();
        let moved = MOVES
            .iter()
            .all(|step| !holds_records(&root.join(step.old)));
        // Only this working copy's own run document: every read passes here,
        // and admission converts the others.
        let execution = root
            .join("run/executions")
            .join(config.execution_context().key())
            .join("sessions")
            .join(format!("{project_id}.json"));
        let current = family_documents(root, project_id)
            .into_iter()
            .chain([(execution, Family::Run)])
            .collect::<Vec<_>>()
            .iter()
            .all(|(path, family)| !family.is_older(path));
        if moved && current {
            return Ok(());
        }
        Err(ReadError::new(
            ReadErrorKind::Interrupted,
            "Saved work is waiting for a one-time upgrade. Start Magistr to finish it.",
        ))
    }

    /// Finish the upgrade for the store and for `project_id`. The caller holds
    /// the project's owner lease; the store-wide moves also hold their own
    /// lock, so two projects admitted at once never move a store twice.
    ///
    /// # Errors
    /// Any I/O failure, an older owner still running, or a saved record its
    /// family codec refuses. Originals stay where they are until their
    /// converted copy is durable.
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
        for step in MOVES {
            move_store(root, step)?;
        }
        // Each codec returns a current document unchanged, so this runs on
        // every admission and needs no marker of its own.
        convert_project(config, project_id)
    }
}

/// What a saved document's first bytes say about its format. Every writer
/// serializes `schema_version` first; writers older than the plan words wrote
/// plan and signal documents with no version, opening with `project_id`.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Head {
    Version(u64),
    Unversioned,
    /// Unreadable or unrecognized: the family's own reader reports it.
    Unknown,
}

fn document_head(path: &Path) -> Head {
    use std::io::Read;
    let mut head = [0u8; 64];
    let Some(read) = fs::File::open(path)
        .ok()
        .and_then(|mut file| file.read(&mut head).ok())
    else {
        return Head::Unknown;
    };
    let Ok(text) = std::str::from_utf8(&head[..read]) else {
        return Head::Unknown;
    };
    let Some(first) = text.trim_start().strip_prefix('{') else {
        return Head::Unknown;
    };
    let first = first.trim_start();
    if let Some(rest) = first.strip_prefix("\"schema_version\"") {
        let digits: String = rest
            .trim_start()
            .trim_start_matches(':')
            .trim_start()
            .chars()
            .take_while(char::is_ascii_digit)
            .collect();
        return digits.parse().map_or(Head::Unknown, Head::Version);
    }
    if first.starts_with("\"project_id\"") {
        return Head::Unversioned;
    }
    Head::Unknown
}

#[derive(Clone, Copy)]
enum Family {
    Plan,
    Decision,
    Signal,
    Run,
}

impl Family {
    /// Whether the document at `path` is positively older than this family's
    /// current format. Unknown content is left to the family's reader.
    fn is_older(self, path: &Path) -> bool {
        match document_head(path) {
            Head::Version(version) => version < self.current_version(),
            Head::Unversioned => matches!(self, Self::Plan | Self::Signal),
            Head::Unknown => false,
        }
    }

    const fn current_version(self) -> u64 {
        match self {
            Self::Plan => PLAN_VERSION,
            Self::Decision => DECISION_VERSION,
            Self::Signal => SIGNALS_VERSION,
            Self::Run => RUN_VERSION,
        }
    }
}

/// This project's one plan, decision, signal and project-wide run document.
fn family_documents(root: &Path, project_id: &str) -> Vec<(PathBuf, Family)> {
    let file = format!("{project_id}.json");
    let document = |family: &str| root.join(family).join("sessions").join(&file);
    vec![
        (document("plan"), Family::Plan),
        (document("decisions"), Family::Decision),
        (document("signal"), Family::Signal),
        (document("run"), Family::Run),
    ]
}

/// This project's documents in every family whose saved references changed
/// words: one plan, decision and signal document, and one run document per
/// working copy's execution.
fn project_documents(root: &Path, project_id: &str) -> Vec<(PathBuf, Family)> {
    let file = format!("{project_id}.json");
    let mut documents = family_documents(root, project_id);
    if let Ok(executions) = fs::read_dir(root.join("run/executions")) {
        let mut runs: Vec<PathBuf> = executions
            .filter_map(Result::ok)
            .map(|execution| execution.path().join("sessions").join(&file))
            .filter(|path| path.exists())
            .collect();
        runs.sort();
        documents.extend(runs.into_iter().map(|path| (path, Family::Run)));
    }
    documents
}

/// Convert one family store into a staging directory, then move each
/// converted record into its new directory and keep the original as a
/// backup. The new directory may already hold lock files (the caller's own
/// owner lease lives there), never records: a record already present must be
/// the byte-identical copy an interrupted earlier attempt published, so a
/// retry finishes the move and never imports twice. An interrupted move
/// leaves the old directory untouched, and readers refuse until it is gone.
fn move_store(root: &Path, step: Move) -> io::Result<()> {
    let old = root.join(step.old);
    if !old.exists() {
        return Ok(());
    }
    // An older owner still running would keep writing into the old directory
    // after the move, and its work would split.
    let live = live_owners(root)?;
    if live.is_empty() && !holds_records(&old) {
        // Opening a store creates its directories; with no records in them
        // there is nothing to move or keep.
        return fs::remove_dir_all(&old);
    }
    if !live.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::WouldBlock,
            format!(
                "Saved work needs a one-time upgrade, but Magistr is still running for {}. Close those sessions, then start Magistr again; nothing was changed.",
                live.join(", ")
            ),
        ));
    }
    // Staged records are written whole and the conversion is deterministic,
    // so an interrupted attempt's staged records are kept and the next
    // attempt writes only what is missing or different: a store of tens of
    // thousands of records finishes even when a client gives up waiting.
    let staging = root.join(format!("{}.upgrading", step.new));
    convert_tree(&old, &staging, Path::new(""), step.codec)?;
    let current = root.join(step.new);
    let staged = files(&staging)?;
    for relative in &staged {
        let target = current.join(relative);
        if let Ok(existing) = fs::read(&target)
            && existing != fs::read(staging.join(relative))?
        {
            return Err(io::Error::other(format!(
                "Saved records exist in both {} and {}. Both are intact; keep the copy you want in {} and retry.",
                old.display(),
                current.display(),
                old.display()
            )));
        }
    }
    for relative in &staged {
        let target = current.join(relative);
        if !target.exists() {
            let parent = target
                .parent()
                .ok_or_else(|| io::Error::other("A saved record has no directory."))?;
            fs::create_dir_all(parent)?;
            fs::rename(staging.join(relative), &target)?;
            sync_directory(parent)?;
        }
    }
    fs::remove_dir_all(&staging)?;
    let backups = root.join("backups");
    fs::create_dir_all(&backups)?;
    let stamp = chrono::Utc::now().format("%Y%m%dT%H%M%SZ");
    fs::rename(
        &old,
        backups.join(format!("{}-before-{}-{stamp}", step.old, step.words)),
    )?;
    sync_directory(root)?;
    sync_directory(&backups)
}

/// Projects whose older owner currently holds its store lock. Nothing
/// current ever locks where older owners did, so every held lock there is
/// someone else's.
fn live_owners(root: &Path) -> io::Result<Vec<String>> {
    let mut live = std::collections::BTreeSet::new();
    for place in OLDER_OWNER_LOCKS {
        let sessions = root.join(place);
        let entries = match fs::read_dir(&sessions) {
            Ok(entries) => entries,
            Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
            Err(error) => return Err(error),
        };
        for entry in entries {
            let name = entry?.file_name().to_string_lossy().into_owned();
            let Some(project) = name
                .strip_suffix("-owner.lock")
                .and_then(|stem| stem.rsplit_once('.'))
                .map(|(project, _role)| project.to_owned())
            else {
                continue;
            };
            let file = fs::File::options()
                .read(true)
                .write(true)
                .open(sessions.join(&name))?;
            if matches!(file.try_lock(), Err(fs::TryLockError::WouldBlock)) {
                live.insert(project);
            }
        }
    }
    Ok(live.into_iter().collect())
}

/// Copy `source` into `target`, converting each family's documents through
/// its own codec and leaving lock files behind. The run store also renames
/// its `receipts/` directories to `check_results/`.
fn convert_tree(source: &Path, target: &Path, relative: &Path, codec: Codec) -> io::Result<()> {
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
            let renamed = match codec {
                Codec::Run if name == "receipts" => "check_results".into(),
                _ => name.clone(),
            };
            convert_tree(&path, &target.join(&renamed), &relative.join(&name), codec)?;
            continue;
        }
        let name = name.to_string_lossy();
        // Locks and owner endpoint sockets belong to running owners, not to
        // the saved work; current owners recreate their own.
        if name.ends_with(".lock") || !kind.is_file() {
            continue;
        }
        let staged = target.join(name.as_ref());
        let bytes = fs::read(&path)?;
        let converted = if !name.ends_with(".json") {
            bytes
        } else {
            let project_id = name.trim_end_matches(".json");
            let in_sessions = relative.ends_with("sessions");
            match codec {
                Codec::Run
                    if relative
                        .components()
                        .any(|part| part.as_os_str() == "receipts") =>
                {
                    magistr_workflow::run::persistence::Persistence::upgrade_check_result(&bytes)
                        .map_err(|error| invalid(&path, error))?
                }
                Codec::Run if in_sessions => {
                    magistr_workflow::run::persistence::Persistence::upgrade_document(
                        &bytes, project_id,
                    )
                    .map_err(|error| invalid(&path, error))?
                }
                Codec::Plan if in_sessions => {
                    magistr_workflow::plan::PlanEngine::upgrade_document(&bytes, project_id)
                        .map_err(|error| invalid(&path, error))?
                }
                // A project's consolidated document; older per-log files
                // (`<project>__<log>.json`) are the older-store import's.
                Codec::Decision if in_sessions && !project_id.contains("__") => {
                    magistr_workflow::decision::persistence::Persistence::upgrade_document(
                        &bytes, project_id,
                    )
                    .map_err(|error| invalid(&path, error))?
                }
                _ => bytes,
            }
        };
        // An earlier attempt's record stays when it is exactly this
        // conversion; anything else is replaced. The directory is made
        // durable once, below, before anything is published from it.
        if fs::read(&staged).ok().as_deref() != Some(converted.as_slice()) {
            stage(&staged, &converted)?;
        }
    }
    sync_directory(target)
}

/// Write one converted record into staging: whole or not at all.
fn stage(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| io::Error::other("A saved record has no directory."))?;
    let mut staged = tempfile::Builder::new()
        .prefix(STAGING_PREFIX)
        .tempfile_in(parent)?;
    staged.write_all(bytes)?;
    staged.as_file().sync_all()?;
    staged.persist(path).map_err(|e| e.error)?;
    Ok(())
}

/// Names a staged record has while it is written; an interrupted write
/// leaves one behind, and it is never published.
const STAGING_PREFIX: &str = ".staging-";

/// Whether `directory` holds a saved record: any regular file other than a
/// lock. Opening a store creates empty directories, lock files and owner
/// endpoint sockets, never records.
fn holds_records(directory: &Path) -> bool {
    let Ok(entries) = fs::read_dir(directory) else {
        return false;
    };
    entries.filter_map(Result::ok).any(|entry| {
        let path = entry.path();
        match entry.file_type() {
            Ok(kind) if kind.is_dir() => holds_records(&path),
            Ok(kind) => kind.is_file() && !entry.file_name().to_string_lossy().ends_with(".lock"),
            Err(_) => true,
        }
    })
}

/// Every regular file under `directory`, relative to it.
fn files(directory: &Path) -> io::Result<Vec<PathBuf>> {
    fn visit(root: &Path, relative: &Path, found: &mut Vec<PathBuf>) -> io::Result<()> {
        for entry in fs::read_dir(root.join(relative))? {
            let entry = entry?;
            let child = relative.join(entry.file_name());
            if entry.file_type()?.is_dir() {
                visit(root, &child, found)?;
            } else if !entry
                .file_name()
                .to_string_lossy()
                .starts_with(STAGING_PREFIX)
            {
                found.push(child);
            }
        }
        Ok(())
    }
    let mut found = Vec::new();
    visit(directory, Path::new(""), &mut found)?;
    found.sort();
    Ok(found)
}

/// Rewrite this project's plan, decision, signal and run documents through their
/// own codecs, each under its document lock, keeping the original beside the
/// backups first.
fn convert_project(config: &NativeConfig, project_id: &str) -> io::Result<()> {
    let root = config.data_dir();
    for (path, family) in project_documents(root, project_id) {
        if !family.is_older(&path) {
            continue;
        }
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
                magistr_workflow::plan::PlanEngine::upgrade_document(&bytes, project_id)
                    .map_err(|error| invalid(&path, error))?
            }
            Family::Signal => {
                magistr_workflow::signal::SignalEngine::upgrade_document(&bytes, project_id)
                    .map_err(|error| invalid(&path, error))?
            }
            Family::Run => magistr_workflow::run::persistence::Persistence::upgrade_document(
                &bytes, project_id,
            )
            .map_err(|error| invalid(&path, error))?,
            Family::Decision => {
                magistr_workflow::decision::persistence::Persistence::upgrade_document(
                    &bytes, project_id,
                )
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
        // Keep the document's place in the store, so two executions' run
        // documents never share a backup.
        let place = path
            .parent()
            .and_then(|sessions| sessions.strip_prefix(root).ok())
            .unwrap_or_else(|| Path::new("workflow"));
        let backup = root
            .join("backups/before-decision-words")
            .join(place)
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
#[path = "store_words_upgrade.tests.rs"]
mod tests;
