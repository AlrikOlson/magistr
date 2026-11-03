//! Canonical check_result payloads and their derived per-run membership index.
//! Every operation uses the same stable ledger lock. Membership precedes
//! canonical publication; dangling membership cannot imply an admitted check.
#[cfg(test)]
#[path = "check_result_ledger.tests.rs"]
mod tests;

use super::check_result_compaction::CheckResultCompaction;
use super::check_result_storage::CheckResultStorage;
use crate::infra::{read_json, transaction::write_locked};
use crate::run::application::{RunError, StepAdmission};
use crate::run::domain::check_result::CheckResult;
use crate::run::domain::check_result_stage::CheckResultStage;
use magistr_workflow_contracts::{Observed, ReadError, ReadErrorKind, ReadSource, WriteError};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

#[derive(Debug, Default)]
struct Memory {
    records: BTreeMap<String, CheckResult>,
    runs: BTreeMap<String, BTreeSet<String>>,
}
impl Memory {
    fn insert(&mut self, check_result: CheckResult) {
        self.runs
            .entry(check_result.target.run_id.clone())
            .or_default()
            .insert(check_result.id.clone());
        self.records.insert(check_result.id.clone(), check_result);
    }
    fn for_run(&self, run: &str) -> Vec<CheckResult> {
        self.runs
            .get(run)
            .into_iter()
            .flatten()
            .filter_map(|id| self.records.get(id).cloned())
            .collect()
    }
}

#[derive(Debug, Clone)]
pub struct CheckResultLedger {
    directory: Option<PathBuf>,
    memory: Arc<Mutex<Memory>>,
    #[cfg(test)]
    fail_stage: Arc<Mutex<Option<CheckResultStage>>>,
}
impl CheckResultLedger {
    #[must_use]
    pub fn in_memory() -> Self {
        Self {
            directory: None,
            memory: Arc::new(Mutex::new(Memory::default())),
            #[cfg(test)]
            fail_stage: Arc::new(Mutex::new(None)),
        }
    }
    #[must_use]
    pub fn on_disk(directory: impl Into<PathBuf>) -> Self {
        Self {
            directory: Some(directory.into()),
            memory: Arc::new(Mutex::new(Memory::default())),
            #[cfg(test)]
            fail_stage: Arc::new(Mutex::new(None)),
        }
    }
    #[must_use]
    pub fn directory(&self) -> Option<&Path> {
        self.directory.as_deref()
    }

    #[cfg(test)]
    pub(crate) fn fail_next_publication(&self, stage: CheckResultStage) {
        *self.fail_stage.lock().unwrap() = Some(stage);
    }
    #[cfg(test)]
    fn check_publication(&self, check_result: &CheckResult) -> Result<(), RunError> {
        let mut failure = self.fail_stage.lock().unwrap();
        if failure.as_ref() == Some(&check_result.stage) {
            *failure = None;
            return Err(before("The check check_result could not be saved."));
        }
        Ok(())
    }

    pub fn admit(
        &self,
        target: &StepAdmission,
        check: &str,
        build: impl FnOnce(u64) -> CheckResult,
    ) -> Result<Observed<CheckResult>, RunError> {
        let make = |records: Vec<CheckResult>| -> Result<CheckResult, RunError> {
            let attempt = records
                .iter()
                .filter(|r| r.target.step_instance == target.step_instance && r.check == check)
                .map(|r| r.attempt)
                .max()
                .unwrap_or(0)
                .checked_add(1)
                .ok_or("The check attempt count is exhausted.")?;
            let check_result = build(attempt);
            validate_id(&check_result.id)?;
            if check_result.target != *target
                || check_result.check != check
                || check_result.attempt != attempt
                || check_result.stage != CheckResultStage::Admitted
            {
                return Err("The check check_result does not match its admission.".into());
            }
            Ok(check_result)
        };
        if let Some(directory) = &self.directory {
            let _lock = lock_directory(directory)?;
            ensure_index(directory)?;
            let check_result = make(read_run(directory, &target.run_id)?)?;
            if read_check_result(directory, &check_result.id)?.is_some() {
                return Err("This check attempt already exists.".into());
            }
            #[cfg(test)]
            self.check_publication(&check_result)?;
            let source = publish(directory, &check_result)?;
            Ok(Observed {
                value: check_result,
                source,
            })
        } else {
            let mut memory = self
                .memory
                .lock()
                .map_err(|_| before("The check history could not be locked."))?;
            let check_result = make(memory.for_run(&target.run_id))?;
            if memory.records.contains_key(&check_result.id) {
                return Err("This check attempt already exists.".into());
            }
            memory.insert(check_result.clone());
            Ok(Observed {
                value: check_result,
                source: ReadSource::Memory,
            })
        }
    }

    /// A missing admission is a refusal, never permission to fabricate one.
    pub(crate) fn update(
        &self,
        id: &str,
        update: impl FnOnce(&mut CheckResult) -> Result<(), RunError>,
    ) -> Result<Observed<CheckResult>, RunError> {
        validate_id(id)?;
        let finish = |found: Option<CheckResult>| -> Result<CheckResult, RunError> {
            let mut check_result =
                found.ok_or("This check was not admitted in the selected check history.")?;
            let original = check_result.clone();
            update(&mut check_result)?;
            if check_result.id != id
                || check_result.target != original.target
                || check_result.attempt != original.attempt
                || check_result.check != original.check
                || check_result.acceptance_revision != original.acceptance_revision
            {
                return Err("The check result changed its admission.".into());
            }
            Ok(check_result)
        };
        if let Some(directory) = &self.directory {
            let _lock = lock_directory(directory)?;
            let check_result = finish(read_check_result(directory, id)?)?;
            #[cfg(test)]
            self.check_publication(&check_result)?;
            let source = publish(directory, &check_result)?;
            Ok(Observed {
                value: check_result,
                source,
            })
        } else {
            let mut memory = self
                .memory
                .lock()
                .map_err(|_| before("The check history could not be locked."))?;
            let check_result = finish(memory.records.get(id).cloned())?;
            memory.insert(check_result.clone());
            Ok(Observed {
                value: check_result,
                source: ReadSource::Memory,
            })
        }
    }

    pub fn observe(&self, id: &str) -> Result<Observed<Option<CheckResult>>, RunError> {
        validate_id(id)?;
        if let Some(directory) = &self.directory {
            let _lock = lock_directory(directory)?;
            observe_check_result(directory, id)
        } else {
            let memory = self
                .memory
                .lock()
                .map_err(|_| before("The check history could not be locked."))?;
            Ok(Observed {
                value: memory.records.get(id).cloned(),
                source: ReadSource::Memory,
            })
        }
    }

    pub fn get(&self, id: &str) -> Result<Option<CheckResult>, RunError> {
        Ok(self.observe(id)?.value)
    }

    pub fn for_run(&self, run_id: &str) -> Result<Vec<CheckResult>, RunError> {
        let mut check_results = if let Some(directory) = &self.directory {
            let _lock = lock_directory(directory)?;
            ensure_index(directory)?;
            read_run(directory, run_id)?
        } else {
            self.memory
                .lock()
                .map_err(|_| before("The check history could not be locked."))?
                .for_run(run_id)
        };
        check_results.sort_by(|a, b| {
            a.admitted_at
                .cmp(&b.admitted_at)
                .then(a.attempt.cmp(&b.attempt))
                .then(a.id.cmp(&b.id))
        });
        Ok(check_results)
    }

    /// Derived membership by command and certified inputs. It survives step and
    /// session changes; canonical check_result payloads remain the only authority.
    pub fn for_check(
        &self,
        request: &crate::run::application::CheckRequest,
        source: &crate::run::application::SourceObservation,
    ) -> Result<Vec<CheckResult>, RunError> {
        let key = reuse_key(
            &request.name,
            &request.check_type,
            request.command.as_deref(),
            Some(source),
        );
        let Some(key) = key else {
            return Ok(Vec::new());
        };
        let mut records = if let Some(directory) = &self.directory {
            let _lock = lock_directory(directory)?;
            ensure_reuse_index(directory)?;
            let bucket = directory.join("checks-v2").join(&key);
            let entries = match fs::read_dir(&bucket) {
                Ok(entries) => entries,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
                Err(error) => return Err(before(format!("Read check index: {error}"))),
            };
            let mut records = Vec::new();
            for entry in entries {
                let path = entry.map_err(|error| before(error.to_string()))?.path();
                if path.extension().is_none_or(|extension| extension != "ref") {
                    continue;
                }
                let id = path
                    .file_stem()
                    .and_then(|id| id.to_str())
                    .ok_or("Invalid check index")?;
                if let Some(check_result) = read_check_result(directory, id)? {
                    if check_result_reuse_key(&check_result).as_deref() != Some(&key) {
                        return Err("Check input index does not match its check_result.".into());
                    }
                    records.push(check_result);
                }
            }
            records
        } else {
            self.memory
                .lock()
                .map_err(|_| before("The check history could not be locked."))?
                .records
                .values()
                .filter(|check_result| {
                    check_result_reuse_key(check_result).as_deref() == Some(&key)
                })
                .cloned()
                .collect()
        };
        records.sort_by(|a, b| {
            a.admitted_at
                .cmp(&b.admitted_at)
                .then(a.attempt.cmp(&b.attempt))
                .then(a.id.cmp(&b.id))
        });
        Ok(records)
    }

    pub(crate) fn compact(
        &self,
        current_run: &str,
        retained: impl FnOnce() -> Result<BTreeSet<String>, String>,
    ) -> Result<CheckResultCompaction, RunError> {
        let mut report = CheckResultCompaction::default();
        if let Some(directory) = &self.directory {
            report.storage = CheckResultStorage::File;
            let _lock = lock_directory(directory)?;
            ensure_index(directory)?;
            let mut retained = retained()?;
            let records = read_directory(directory)?;
            retain_reuse_origins(records.values(), &mut retained);
            for check_result in records.into_values() {
                if report.retain(&check_result, current_run, &retained) {
                    continue;
                }
                fs::remove_file(check_result_path(directory, &check_result.id)?).map_err(|e| {
                    let error = before(format!(
                        "The old check check_result could not be removed: {e}"
                    ));
                    if report.removed > 0 {
                        error.after_commit()
                    } else {
                        error
                    }
                })?;
                // The first successful unlink is already an effect, including
                // when its directory synchronization subsequently fails.
                sync_directory(directory).map_err(RunError::after_commit)?;
                remove_membership(
                    &run_directory(directory, &check_result.target.run_id),
                    &check_result.id,
                )
                .map_err(RunError::after_commit)?;
                if let Some(key) = check_result_reuse_key(&check_result) {
                    remove_membership(&directory.join("checks-v2").join(key), &check_result.id)?;
                }
                report.removed += 1;
            }
        } else {
            let mut memory = self
                .memory
                .lock()
                .map_err(|_| before("The check history could not be locked."))?;
            let mut retained = retained()?;
            retain_reuse_origins(memory.records.values(), &mut retained);
            let remove: Vec<_> = memory
                .records
                .values()
                .filter(|r| !report.retain(r, current_run, &retained))
                .map(|r| r.id.clone())
                .collect();
            for id in remove {
                if let Some(check_result) = memory.records.remove(&id) {
                    if let Some(ids) = memory.runs.get_mut(&check_result.target.run_id) {
                        ids.remove(&id);
                    }
                    report.removed += 1;
                }
            }
        }
        Ok(report)
    }
}

fn before(message: impl Into<String>) -> RunError {
    WriteError::before_commit(message).into()
}

fn validate_id(id: &str) -> Result<(), RunError> {
    if id.is_empty()
        || !id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
    {
        return Err("invalid check_result ID".into());
    }
    Ok(())
}

fn check_result_path(directory: &Path, id: &str) -> Result<PathBuf, RunError> {
    validate_id(id)?;
    Ok(directory.join(format!("{id}.json")))
}

fn run_directory(directory: &Path, run: &str) -> PathBuf {
    directory
        .join("runs-v1")
        .join(hex::encode(Sha256::digest(run.as_bytes())))
}

fn lock_directory(directory: &Path) -> Result<File, RunError> {
    fs::create_dir_all(directory)
        .map_err(|e| before(format!("create check_result ledger: {e}")))?;
    let lock = OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(directory.join(".lock"))
        .map_err(|e| before(format!("open check_result ledger lock: {e}")))?;
    lock.lock()
        .map_err(|e| before(format!("lock check_result ledger: {e}")))?;
    Ok(lock)
}

fn read_check_result(directory: &Path, id: &str) -> Result<Option<CheckResult>, RunError> {
    Ok(observe_check_result(directory, id)?.value)
}

fn observe_check_result(
    directory: &Path,
    id: &str,
) -> Result<Observed<Option<CheckResult>>, RunError> {
    let read: Observed<Option<CheckResult>> = read_json(&check_result_path(directory, id)?)?;
    if let Some(check_result) = &read.value {
        if check_result.id != id {
            return Err(ReadError::new(
                ReadErrorKind::Identity,
                "The check check_result belongs to another attempt.",
            )
            .into());
        }
        let complete = check_result.outcome.is_some()
            && check_result.recorded_at.is_some()
            && check_result.duration_ms.is_some();
        let valid = match check_result.stage {
            CheckResultStage::Admitted => {
                check_result.outcome.is_none()
                    && check_result.recorded_at.is_none()
                    && check_result.settlement.is_none()
            }
            CheckResultStage::Recorded | CheckResultStage::Applying => {
                complete && check_result.settlement.is_none() && check_result.settled_at.is_none()
            }
            CheckResultStage::Settled => {
                complete && check_result.settlement.is_some() && check_result.settled_at.is_some()
            }
        };
        if !valid {
            return Err(ReadError::new(
                ReadErrorKind::Schema,
                "The check check_result is incomplete.",
            )
            .into());
        }
    }
    Ok(read)
}

fn read_directory(directory: &Path) -> Result<BTreeMap<String, CheckResult>, RunError> {
    let mut records = BTreeMap::new();
    for entry in
        fs::read_dir(directory).map_err(|e| before(format!("read check_result ledger: {e}")))?
    {
        let path = entry
            .map_err(|e| before(format!("read check_result entry: {e}")))?
            .path();
        if path.extension().is_none_or(|e| e != "json") {
            continue;
        }
        let id = path
            .file_stem()
            .and_then(|s| s.to_str())
            .ok_or("invalid check_result filename")?;
        if let Some(check_result) = read_check_result(directory, id)? {
            records.insert(check_result.id.clone(), check_result);
        }
    }
    Ok(records)
}

fn ensure_index(directory: &Path) -> Result<(), RunError> {
    let marker = directory.join("runs-v1").join("ready");
    match fs::read(&marker) {
        Ok(bytes) if bytes == b"1\n" => return Ok(()),
        Ok(_) => return Err("unsupported check_result index version".into()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => return Err(before(format!("read check_result index: {e}"))),
    }
    for check_result in read_directory(directory)?.into_values() {
        add_membership(directory, &check_result)?;
    }
    fs::create_dir_all(directory.join("runs-v1"))
        .map_err(|e| before(format!("create check_result index: {e}")))?;
    sync_directory(directory)?;
    // A crash before publication leaves this derived index rebuild retryable.
    publish_bytes(&directory.join("runs-v1"), "ready", b"1\n")
}

fn add_membership(directory: &Path, check_result: &CheckResult) -> Result<(), RunError> {
    validate_id(&check_result.id)?;
    let bucket = run_directory(directory, &check_result.target.run_id);
    fs::create_dir_all(&bucket)
        .map_err(|e| before(format!("create check_result run index: {e}")))?;
    let member = bucket.join(format!("{}.ref", check_result.id));
    // Idempotent also after a partial index build or interrupted admission.
    OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(member)
        .and_then(|file| file.sync_all())
        .map_err(|e| before(format!("persist check_result membership: {e}")))?;
    sync_directory(&bucket)?;
    sync_directory(&directory.join("runs-v1"))?;
    sync_directory(directory)
}

fn read_run(directory: &Path, run: &str) -> Result<Vec<CheckResult>, RunError> {
    let mut check_results = Vec::new();
    let entries = match fs::read_dir(run_directory(directory, run)) {
        Ok(entries) => entries,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(check_results),
        Err(e) => return Err(before(format!("read check_result run index: {e}"))),
    };
    for entry in entries {
        let path = entry
            .map_err(|e| before(format!("read check_result membership: {e}")))?
            .path();
        if path.extension().is_none_or(|e| e != "ref") {
            continue;
        }
        let id = path
            .file_stem()
            .and_then(|s| s.to_str())
            .ok_or("invalid check_result membership")?;
        if let Some(check_result) = read_check_result(directory, id)? {
            if check_result.target.run_id != run {
                return Err("check_result run index mismatch".into());
            }
            check_results.push(check_result);
        }
    }
    Ok(check_results)
}

fn remove_membership(bucket: &Path, id: &str) -> Result<(), RunError> {
    match fs::remove_file(bucket.join(format!("{id}.ref"))) {
        Ok(()) => sync_directory(bucket)?,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => return Err(before(format!("remove check_result membership: {e}"))),
    }
    match fs::remove_dir(bucket) {
        Ok(()) => sync_directory(bucket.parent().expect("run bucket has a parent")),
        Err(e)
            if matches!(
                e.kind(),
                std::io::ErrorKind::DirectoryNotEmpty | std::io::ErrorKind::NotFound
            ) =>
        {
            Ok(())
        }
        Err(e) => Err(before(format!("remove empty check_result run index: {e}"))),
    }
}

fn publish(directory: &Path, check_result: &CheckResult) -> Result<ReadSource, RunError> {
    let existing = check_result_path(directory, &check_result.id)?.is_file();
    let run_member = run_directory(directory, &check_result.target.run_id)
        .join(format!("{}.ref", check_result.id));
    if !existing || !run_member.is_file() {
        add_membership(directory, check_result)?;
    }
    add_reuse_membership(directory, check_result, existing)?;
    let stamp = write_locked(
        &check_result_path(directory, &check_result.id)?,
        check_result,
    )?;
    Ok(ReadSource::File(stamp))
}

// The small derived index marker is also synced before its acknowledgement.
fn publish_bytes(directory: &Path, name: &str, bytes: &[u8]) -> Result<(), RunError> {
    let temporary = directory.join(format!(".{name}-{}.tmp", std::process::id()));
    let mut file = OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .open(&temporary)
        .map_err(|e| before(format!("The check index could not be opened: {e}")))?;
    file.write_all(bytes)
        .and_then(|()| file.sync_all())
        .map_err(|e| before(format!("The check index could not be saved: {e}")))?;
    fs::rename(&temporary, directory.join(name))
        .map_err(|e| before(format!("The check index could not be saved: {e}")))?;
    sync_directory(directory)
}

#[cfg(unix)]
fn sync_directory(directory: &Path) -> Result<(), RunError> {
    File::open(directory)
        .and_then(|dir| dir.sync_all())
        .map_err(|e| before(format!("The check history could not be synchronized: {e}")))
}
#[cfg(not(unix))]
fn sync_directory(_directory: &Path) -> Result<(), RunError> {
    Ok(())
}

fn reuse_key(
    check: &str,
    check_type: &crate::run::domain::check::CheckType,
    command: Option<&str>,
    source: Option<&crate::run::application::SourceObservation>,
) -> Option<String> {
    let source = source?;
    if source.execution_digest.is_empty() {
        return None;
    }
    let key = serde_json::to_vec(&(
        check,
        check_type,
        command?,
        &source.workspace_root,
        &source.scheme,
        &source.config_digest,
        &source.execution_digest,
    ))
    .ok()?;
    Some(hex::encode(Sha256::digest(key)))
}

fn check_result_reuse_key(check_result: &CheckResult) -> Option<String> {
    reuse_key(
        &check_result.check,
        &check_result.check_type,
        check_result.command.as_deref(),
        check_result.source.as_ref(),
    )
}

fn add_reuse_membership(
    directory: &Path,
    check_result: &CheckResult,
    existing: bool,
) -> Result<(), RunError> {
    let Some(key) = check_result_reuse_key(check_result) else {
        return Ok(());
    };
    let bucket = directory.join("checks-v2").join(key);
    let member = bucket.join(format!("{}.ref", check_result.id));
    if existing && member.is_file() {
        return Ok(());
    }
    fs::create_dir_all(&bucket).map_err(|error| before(error.to_string()))?;
    OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(member)
        .and_then(|file| file.sync_all())
        .map_err(|error| before(error.to_string()))?;
    sync_directory(&bucket)?;
    sync_directory(&directory.join("checks-v2"))?;
    sync_directory(directory)
}

fn ensure_reuse_index(directory: &Path) -> Result<(), RunError> {
    let marker = directory.join("checks-v2/ready");
    match fs::read(&marker) {
        Ok(bytes) if bytes == b"1\n" => return Ok(()),
        Ok(_) => return Err("Unsupported check input index version.".into()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(before(error.to_string())),
    }
    for check_result in read_directory(directory)?.into_values() {
        add_reuse_membership(directory, &check_result, false)?;
    }
    fs::create_dir_all(directory.join("checks-v2")).map_err(|error| before(error.to_string()))?;
    sync_directory(directory)?;
    publish_bytes(&directory.join("checks-v2"), "ready", b"1\n")
}

fn retain_reuse_origins<'a>(
    check_results: impl Iterator<Item = &'a CheckResult>,
    retained: &mut BTreeSet<String>,
) {
    for check_result in check_results {
        if let Some(crate::run::application::CheckExecution::Reused { check_result_id }) =
            check_result.execution()
        {
            retained.insert(check_result_id.clone());
        }
    }
}
