//! The one-time move of saved work to the glossary's vocabulary.
//!
//! Each family whose words changed moves its store directory once, in a fixed
//! order: the run store from `ship/` to `run/` (its check results from
//! `receipts/` to `check_results/`), then the plan store from `roadmap/` to
//! `plan/`, then the decision store from `think/` to `decisions/`, then the
//! request store from `signal/` to `requests/`. Each project's plan,
//! decision, request and run documents then convert through their own
//! codecs (a run document's check results first), which decide what is old
//! from the document's version, never from its references: after the plan
//! words `task:` names a plan task, while before the run words it named a
//! run step. This module only owns where saved work lives and the order of
//! the moves. Admission finishes them before any engine opens; readers
//! refuse while one is unfinished.

use super::NativeConfig;
use magistr_workflow::infra::AtomicFile;
use magistr_workflow_contracts::{ReadError, ReadErrorKind};
use std::{
    fs, io,
    path::{Path, PathBuf},
};

/// Converts saved work to the glossary's vocabulary exactly once.
pub struct StoreWordsUpgrade;

const LOCK: &str = "store-words-upgrade.lock";

/// The words of each family's latest in-place document conversion. A
/// document converted in place keeps its original under
/// `backups/before-<words>/`, one folder per conversion, so an installation
/// upgraded through several keeps each original. The check words converted
/// run documents and their check results; the team words were the last to
/// convert the other families.
const RUN_DOCUMENT_WORDS: &str = "check-words";
const DOCUMENT_WORDS: &str = "team-words";

/// One family store directory that moves to its new name.
#[derive(Clone, Copy)]
struct Move {
    old: &'static str,
    new: &'static str,
    words: &'static str,
    codec: Codec,
}

/// The order is the glossary's: the run store has finished before the plan
/// store converts its references, the plan store before the decision store,
/// and the decision store before the request store.
const MOVES: [Move; 4] = [
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
    Move {
        old: "signal",
        new: "requests",
        words: "request-words",
        codec: Codec::Request,
    },
];

/// Where older owners hold their project locks: before the plan words in the
/// old plan directory, until the decision words in the plan directory, and
/// until the request words under the decision words' lease directory.
/// Current owners lock under [`super::owner_lease::OWNER_LEASES`], so every
/// held lock in these is an older owner's.
const OLDER_OWNER_LOCKS: [&str; 3] = ["roadmap/sessions", "plan/sessions", "owners/decision-words"];

#[derive(Clone, Copy)]
enum Codec {
    Run,
    Plan,
    Decision,
    Request,
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
/// plan and request documents with no version, opening with `project_id`.
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
    Request,
    Run,
}

impl Family {
    /// Whether the document at `path` is positively older than this family's
    /// current format. Unknown content is left to the family's reader.
    fn is_older(self, path: &Path) -> bool {
        match document_head(path) {
            Head::Version(version) => version < self.current_version(),
            Head::Unversioned => matches!(self, Self::Plan | Self::Request),
            Head::Unknown => false,
        }
    }

    /// The words of this family's latest in-place conversion.
    fn document_words(self) -> &'static str {
        match self {
            Self::Run => RUN_DOCUMENT_WORDS,
            Self::Plan | Self::Decision | Self::Request => DOCUMENT_WORDS,
        }
    }

    /// Each family's codec owns its current version; this only asks it.
    fn current_version(self) -> u64 {
        u64::from(match self {
            Self::Plan => magistr_workflow_contracts::PLAN_SCHEMA_VERSION,
            Self::Decision => magistr_workflow::decision::persistence::Persistence::CURRENT_VERSION,
            Self::Request => magistr_workflow_contracts::REQUESTS_SCHEMA_VERSION,
            Self::Run => magistr_workflow::run::persistence::Persistence::CURRENT_VERSION,
        })
    }
}

/// This project's one plan, decision, request and project-wide run document.
fn family_documents(root: &Path, project_id: &str) -> Vec<(PathBuf, Family)> {
    let file = format!("{project_id}.json");
    let document = |family: &str| root.join(family).join("sessions").join(&file);
    vec![
        (document("plan"), Family::Plan),
        (document("decisions"), Family::Decision),
        (document("requests"), Family::Request),
        (document("run"), Family::Run),
    ]
}

/// This project's documents in every family whose saved references changed
/// words: one plan, decision and request document, and one run document per
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

/// Convert one family store into a staging directory, publish the staged
/// store under its new name, and keep the original as a backup. Staging is
/// where the time goes, so its records convert on several threads; a staged
/// store whose new directory does not exist yet is published by one rename.
/// The new directory may already hold lock files, never records: a record
/// already present must be the byte-identical copy an interrupted earlier
/// attempt published, so a retry finishes the move and never imports twice.
/// An interrupted move leaves the old directory untouched, and readers
/// refuse until it is gone.
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
        return fs::remove_dir_all(&old).map_err(|error| invalid(&old, error));
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
    let store = StoreSurvey::of(&old, step.codec)?;
    let staging = root.join(format!("{}.upgrading", step.new));
    store.stage(&old, &staging)?;
    let current = root.join(step.new);
    // A directory another program is reading refuses the rename; the staged
    // records then move one by one, as they do into an existing directory.
    if current.exists() || AtomicFile::rename(&staging, &current).is_err() {
        store.merge(&old, &staging, &current)?;
        fs::remove_dir_all(&staging).map_err(|error| invalid(&staging, error))?;
    }
    sync_directory(root)?;
    let backups = root.join("backups");
    fs::create_dir_all(&backups)?;
    let stamp = chrono::Utc::now().format("%Y%m%dT%H%M%SZ");
    let backup = backups.join(format!("{}-before-{}-{stamp}", step.old, step.words));
    AtomicFile::rename(&old, &backup).map_err(|error| invalid(&old, error))?;
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

/// How one saved file changes on its way to the store's new name.
enum Conversion {
    /// Carried over byte for byte.
    Unchanged,
    CheckResult,
    /// A project's document, through its family's codec.
    Document(Codec, String),
}

/// One saved file of a family store: where it is in the old store, where it
/// goes in the new one, and how its content changes.
struct Record {
    source: PathBuf,
    target: PathBuf,
    conversion: Conversion,
    /// Whether the old store's directory listing says the file holds nothing.
    empty: bool,
}

impl Record {
    /// An empty file carried over as it is: more than half of a run store
    /// is empty index markers.
    fn carries_nothing(&self) -> bool {
        self.empty && matches!(self.conversion, Conversion::Unchanged)
    }

    fn converted(&self, bytes: Vec<u8>, path: &Path) -> io::Result<Vec<u8>> {
        use magistr_workflow::{decision, plan::PlanEngine, request::RequestEngine, run};
        match &self.conversion {
            Conversion::Unchanged => Ok(bytes),
            Conversion::CheckResult => run::persistence::Persistence::upgrade_check_result(&bytes)
                .map_err(|error| invalid(path, error)),
            Conversion::Document(Codec::Run, project) => {
                run::persistence::Persistence::upgrade_document(&bytes, project)
                    .map_err(|error| invalid(path, error))
            }
            Conversion::Document(Codec::Plan, project) => {
                PlanEngine::upgrade_document(&bytes, project).map_err(|error| invalid(path, error))
            }
            Conversion::Document(Codec::Decision, project) => {
                decision::persistence::Persistence::upgrade_document(&bytes, project)
                    .map_err(|error| invalid(path, error))
            }
            Conversion::Document(Codec::Request, project) => {
                RequestEngine::upgrade_document(&bytes, project)
                    .map_err(|error| invalid(path, error))
            }
        }
    }
}

/// Everything one family store holds, read once before anything is written:
/// its directories and its records, each with its place in the new store.
/// Lock files and owner endpoint sockets belong to running owners, not to
/// the saved work, and are left behind; current owners recreate their own.
/// The run store's `receipts/` directories become `check_results/`.
struct StoreSurvey {
    directories: Vec<PathBuf>,
    records: Vec<Record>,
}

impl StoreSurvey {
    fn of(old: &Path, codec: Codec) -> io::Result<Self> {
        let mut store = Self {
            directories: Vec::new(),
            records: Vec::new(),
        };
        store.visit(old, Path::new(""), Path::new(""), codec)?;
        Ok(store)
    }

    fn visit(&mut self, old: &Path, source: &Path, target: &Path, codec: Codec) -> io::Result<()> {
        self.directories.push(target.to_path_buf());
        let directory = old.join(source);
        let in_sessions = source.ends_with("sessions");
        let in_receipts = source
            .components()
            .any(|part| part.as_os_str() == "receipts");
        for entry in fs::read_dir(&directory).map_err(|error| invalid(&directory, error))? {
            let entry = entry.map_err(|error| invalid(&directory, error))?;
            let path = entry.path();
            let kind = entry.file_type().map_err(|error| invalid(&path, error))?;
            let name = entry.file_name();
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
                self.visit(old, &source.join(&name), &target.join(renamed), codec)?;
                continue;
            }
            let text = name.to_string_lossy();
            if text.ends_with(".lock") || !kind.is_file() {
                continue;
            }
            let project = text.trim_end_matches(".json");
            let conversion = if !text.ends_with(".json") {
                Conversion::Unchanged
            } else {
                match codec {
                    Codec::Run if in_receipts => Conversion::CheckResult,
                    // A project's consolidated decision document; older
                    // per-log files (`<project>__<log>.json`) are the
                    // older-store import's.
                    Codec::Decision if in_sessions && project.contains("__") => {
                        Conversion::Unchanged
                    }
                    _ if in_sessions => Conversion::Document(codec, project.to_owned()),
                    _ => Conversion::Unchanged,
                }
            };
            let empty = entry
                .metadata()
                .map_err(|error| invalid(&path, error))?
                .len()
                == 0;
            self.records.push(Record {
                source: source.join(&name),
                target: target.join(&name),
                conversion,
                empty,
            });
        }
        Ok(())
    }

    /// Fill `staging` with exactly this store, converted. The time goes to
    /// creating files and to waiting for the disk, so each record costs as
    /// few file operations as its safety allows, and the waits are made
    /// together at the end, where they overlap. Nothing reads a staged
    /// record until the whole staged store is published, so each is written
    /// in place; every one is confirmed by the disk before this returns.
    /// The conversion is deterministic, so a resumed attempt keeps the
    /// staged records that already match and writes only what is missing or
    /// different. Anything else an earlier attempt left there is removed
    /// first, so the staged store never holds more than the old one does.
    fn stage(&self, old: &Path, staging: &Path) -> io::Result<()> {
        let resumed = staging.exists();
        if resumed {
            self.prune(staging)?;
        }
        for directory in &self.directories {
            let directory = staging.join(directory);
            fs::create_dir_all(&directory).map_err(|error| invalid(&directory, error))?;
        }
        let writers = std::thread::available_parallelism()
            .map_or(1, usize::from)
            .min(WRITE_THREADS);
        on_every(&self.records, writers, |record| {
            let staged = staging.join(&record.target);
            if record.carries_nothing() {
                // An empty file cannot be half-written: it needs no read of
                // its source and no wait for the disk.
                return stage_empty(&staged).map_err(|error| invalid(&staged, error));
            }
            let source = old.join(&record.source);
            let bytes = fs::read(&source).map_err(|error| invalid(&source, error))?;
            let converted = record.converted(bytes, &source)?;
            if resumed && fs::read(&staged).ok().as_deref() == Some(converted.as_slice()) {
                return Ok(());
            }
            AtomicFile::write_in_place(&staged, &converted).map_err(|error| invalid(&staged, error))
        })?;
        // A record an interrupted attempt wrote may never have reached the
        // disk, so every record with content is confirmed, not only the ones
        // written just now.
        let written: Vec<&Record> = self
            .records
            .iter()
            .filter(|record| !record.carries_nothing())
            .collect();
        on_every(&written, CONFIRM_THREADS, |record| {
            let staged = staging.join(&record.target);
            AtomicFile::confirm(&staged).map_err(|error| invalid(&staged, error))
        })?;
        for directory in &self.directories {
            sync_directory(&staging.join(directory))?;
        }
        Ok(())
    }

    /// Remove what `staging` holds beyond this store's own records.
    fn prune(&self, staging: &Path) -> io::Result<()> {
        let directories: std::collections::HashSet<&Path> =
            self.directories.iter().map(PathBuf::as_path).collect();
        let records: std::collections::HashSet<&Path> = self
            .records
            .iter()
            .map(|record| record.target.as_path())
            .collect();
        let mut pending = vec![PathBuf::new()];
        while let Some(relative) = pending.pop() {
            let directory = staging.join(&relative);
            for entry in fs::read_dir(&directory).map_err(|error| invalid(&directory, error))? {
                let entry = entry.map_err(|error| invalid(&directory, error))?;
                let child = relative.join(entry.file_name());
                let path = entry.path();
                let kind = entry.file_type().map_err(|error| invalid(&path, error))?;
                if kind.is_dir() && directories.contains(child.as_path()) {
                    pending.push(child);
                } else if kind.is_dir() {
                    fs::remove_dir_all(&path).map_err(|error| invalid(&path, error))?;
                } else if !records.contains(child.as_path()) {
                    fs::remove_file(&path).map_err(|error| invalid(&path, error))?;
                }
            }
        }
        Ok(())
    }

    /// Move the staged records into a new directory that already exists.
    /// Every destination is checked before the first record moves.
    fn merge(&self, old: &Path, staging: &Path, current: &Path) -> io::Result<()> {
        for record in &self.records {
            let target = current.join(&record.target);
            let staged = staging.join(&record.target);
            if let Ok(existing) = fs::read(&target)
                && existing != fs::read(&staged).map_err(|error| invalid(&staged, error))?
            {
                return Err(io::Error::other(format!(
                    "Saved records exist in both {} and {}. Both are intact; keep the copy you want in {} and retry.",
                    old.display(),
                    current.display(),
                    old.display()
                )));
            }
        }
        for record in &self.records {
            let target = current.join(&record.target);
            if !target.exists() {
                let parent = target
                    .parent()
                    .ok_or_else(|| io::Error::other("A saved record has no directory."))?;
                fs::create_dir_all(parent).map_err(|error| invalid(parent, error))?;
                AtomicFile::rename(&staging.join(&record.target), &target)
                    .map_err(|error| invalid(&target, error))?;
                sync_directory(parent)?;
            }
        }
        Ok(())
    }
}

/// Leave an empty file at `staged`: one file operation when nothing is
/// there yet.
fn stage_empty(staged: &Path) -> io::Result<()> {
    match fs::File::options()
        .write(true)
        .create_new(true)
        .open(staged)
    {
        Ok(_) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
            if fs::metadata(staged)?.len() == 0 {
                Ok(())
            } else {
                fs::write(staged, b"")
            }
        }
        Err(error) => Err(error),
    }
}

/// Run `work` on every item from `threads` threads. The first failure stops
/// the others and is the result.
fn on_every<T: Sync>(
    items: &[T],
    threads: usize,
    work: impl Fn(&T) -> io::Result<()> + Sync,
) -> io::Result<()> {
    use std::sync::{
        Mutex, PoisonError,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    };
    let next = AtomicUsize::new(0);
    let stopped = AtomicBool::new(false);
    let failure = Mutex::new(None);
    std::thread::scope(|scope| {
        for _ in 0..threads.clamp(1, items.len().max(1)) {
            scope.spawn(|| {
                while !stopped.load(Ordering::Relaxed) {
                    let Some(item) = items.get(next.fetch_add(1, Ordering::Relaxed)) else {
                        break;
                    };
                    if let Err(error) = work(item) {
                        stopped.store(true, Ordering::Relaxed);
                        failure
                            .lock()
                            .unwrap_or_else(PoisonError::into_inner)
                            .get_or_insert(error);
                    }
                }
            });
        }
    });
    failure
        .into_inner()
        .unwrap_or_else(PoisonError::into_inner)
        .map_or(Ok(()), Err)
}

/// Converting and creating records is bounded by the processors and by the
/// file system, which takes one new file at a time.
const WRITE_THREADS: usize = 8;
/// A confirmation only waits for the disk, and one answer from the disk
/// settles every wait in flight: measured on 15,000 records, 64 waits at a
/// time finish in a quarter of the time 8 take.
const CONFIRM_THREADS: usize = 64;

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

/// Rewrite this project's plan, decision, request and run documents through their
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
        if matches!(family, Family::Run) {
            // The document's version says its check results are current, so
            // they convert first: an interrupted attempt leaves the document
            // older and the next admission finishes them.
            convert_check_results(root, &path, project_id)?;
        }
        let converted = match family {
            Family::Plan => {
                magistr_workflow::plan::PlanEngine::upgrade_document(&bytes, project_id)
                    .map_err(|error| invalid(&path, error))?
            }
            Family::Request => {
                magistr_workflow::request::RequestEngine::upgrade_document(&bytes, project_id)
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
            .join(format!("backups/before-{}", family.document_words()))
            .join(place)
            .join(name);
        if !backup.exists() {
            publish(&backup, &bytes, false)?;
        }
        publish(&path, &converted, true)?;
    }
    Ok(())
}

/// Convert the check results beside one run document through the run codec,
/// keeping each original under the backups first. They live in the
/// document's store as `check_results/<project>/`.
fn convert_check_results(root: &Path, document: &Path, project_id: &str) -> io::Result<()> {
    let Some(store) = document.parent().and_then(Path::parent) else {
        return Ok(());
    };
    let directory = store.join("check_results").join(project_id);
    let records = match fs::read_dir(&directory) {
        Ok(entries) => entries,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error),
    };
    let mut paths: Vec<PathBuf> = records
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.extension()
                .is_some_and(|extension| extension == "json")
        })
        .collect();
    paths.sort();
    for path in paths {
        let bytes = fs::read(&path)?;
        let converted =
            magistr_workflow::run::persistence::Persistence::upgrade_check_result(&bytes)
                .map_err(|error| invalid(&path, error))?;
        if converted == bytes {
            continue;
        }
        let place = path.strip_prefix(root).unwrap_or(&path);
        let backup = root
            .join(format!("backups/before-{RUN_DOCUMENT_WORDS}"))
            .join(place);
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
    fs::create_dir_all(parent).map_err(|error| invalid(parent, error))?;
    if replace {
        AtomicFile::replace(path, bytes)
    } else {
        AtomicFile::create(path, bytes)
    }
    .map_err(|error| invalid(path, error))?;
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
