//! Canonical receipt payloads and their derived per-run membership index.
//! Every operation uses the same stable ledger lock. Membership precedes
//! canonical publication; dangling membership cannot imply an admitted check.
#[cfg(test)]
#[path = "receipt_ledger.tests.rs"]
mod tests;

use super::receipt_compaction::ReceiptCompaction;
use super::receipt_storage::ReceiptStorage;
use crate::infra::{read_json, transaction::write_locked};
use crate::ship::application::{ShipError, TaskAdmission};
use crate::ship::domain::check_receipt::CheckReceipt;
use crate::ship::domain::receipt_stage::ReceiptStage;
use magistr_workflow_contracts::{Observed, ReadError, ReadErrorKind, ReadSource, WriteError};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

#[derive(Debug, Default)]
struct Memory {
    records: BTreeMap<String, CheckReceipt>,
    runs: BTreeMap<String, BTreeSet<String>>,
}
impl Memory {
    fn insert(&mut self, receipt: CheckReceipt) {
        self.runs
            .entry(receipt.target.run_id.clone())
            .or_default()
            .insert(receipt.id.clone());
        self.records.insert(receipt.id.clone(), receipt);
    }
    fn for_run(&self, run: &str) -> Vec<CheckReceipt> {
        self.runs
            .get(run)
            .into_iter()
            .flatten()
            .filter_map(|id| self.records.get(id).cloned())
            .collect()
    }
}

#[derive(Debug, Clone)]
pub struct ReceiptLedger {
    directory: Option<PathBuf>,
    memory: Arc<Mutex<Memory>>,
    #[cfg(test)]
    fail_stage: Arc<Mutex<Option<ReceiptStage>>>,
}
impl ReceiptLedger {
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
    pub(crate) fn fail_next_publication(&self, stage: ReceiptStage) {
        *self.fail_stage.lock().unwrap() = Some(stage);
    }
    #[cfg(test)]
    fn check_publication(&self, receipt: &CheckReceipt) -> Result<(), ShipError> {
        let mut failure = self.fail_stage.lock().unwrap();
        if failure.as_ref() == Some(&receipt.stage) {
            *failure = None;
            return Err(before("The check receipt could not be saved."));
        }
        Ok(())
    }

    pub fn admit(
        &self,
        target: &TaskAdmission,
        gate: &str,
        build: impl FnOnce(u64) -> CheckReceipt,
    ) -> Result<Observed<CheckReceipt>, ShipError> {
        let make = |records: Vec<CheckReceipt>| -> Result<CheckReceipt, ShipError> {
            let attempt = records
                .iter()
                .filter(|r| r.target.task_instance == target.task_instance && r.gate == gate)
                .map(|r| r.attempt)
                .max()
                .unwrap_or(0)
                .checked_add(1)
                .ok_or("The check attempt count is exhausted.")?;
            let receipt = build(attempt);
            validate_id(&receipt.id)?;
            if receipt.target != *target
                || receipt.gate != gate
                || receipt.attempt != attempt
                || receipt.stage != ReceiptStage::Admitted
            {
                return Err("The check receipt does not match its admission.".into());
            }
            Ok(receipt)
        };
        if let Some(directory) = &self.directory {
            let _lock = lock_directory(directory)?;
            ensure_index(directory)?;
            let receipt = make(read_run(directory, &target.run_id)?)?;
            if read_receipt(directory, &receipt.id)?.is_some() {
                return Err("This check attempt already exists.".into());
            }
            #[cfg(test)]
            self.check_publication(&receipt)?;
            let source = publish(directory, &receipt)?;
            Ok(Observed {
                value: receipt,
                source,
            })
        } else {
            let mut memory = self
                .memory
                .lock()
                .map_err(|_| before("The check history could not be locked."))?;
            let receipt = make(memory.for_run(&target.run_id))?;
            if memory.records.contains_key(&receipt.id) {
                return Err("This check attempt already exists.".into());
            }
            memory.insert(receipt.clone());
            Ok(Observed {
                value: receipt,
                source: ReadSource::Memory,
            })
        }
    }

    /// A missing admission is a refusal, never permission to fabricate one.
    pub(crate) fn update(
        &self,
        id: &str,
        update: impl FnOnce(&mut CheckReceipt) -> Result<(), ShipError>,
    ) -> Result<Observed<CheckReceipt>, ShipError> {
        validate_id(id)?;
        let finish = |found: Option<CheckReceipt>| -> Result<CheckReceipt, ShipError> {
            let mut receipt =
                found.ok_or("This check was not admitted in the selected check history.")?;
            let original = receipt.clone();
            update(&mut receipt)?;
            if receipt.id != id
                || receipt.target != original.target
                || receipt.attempt != original.attempt
                || receipt.gate != original.gate
                || receipt.acceptance_revision != original.acceptance_revision
            {
                return Err("The check result changed its admission.".into());
            }
            Ok(receipt)
        };
        if let Some(directory) = &self.directory {
            let _lock = lock_directory(directory)?;
            let receipt = finish(read_receipt(directory, id)?)?;
            #[cfg(test)]
            self.check_publication(&receipt)?;
            let source = publish(directory, &receipt)?;
            Ok(Observed {
                value: receipt,
                source,
            })
        } else {
            let mut memory = self
                .memory
                .lock()
                .map_err(|_| before("The check history could not be locked."))?;
            let receipt = finish(memory.records.get(id).cloned())?;
            memory.insert(receipt.clone());
            Ok(Observed {
                value: receipt,
                source: ReadSource::Memory,
            })
        }
    }

    pub fn observe(&self, id: &str) -> Result<Observed<Option<CheckReceipt>>, ShipError> {
        validate_id(id)?;
        if let Some(directory) = &self.directory {
            let _lock = lock_directory(directory)?;
            observe_receipt(directory, id)
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

    pub fn get(&self, id: &str) -> Result<Option<CheckReceipt>, ShipError> {
        Ok(self.observe(id)?.value)
    }

    pub fn for_run(&self, run_id: &str) -> Result<Vec<CheckReceipt>, ShipError> {
        let mut receipts = if let Some(directory) = &self.directory {
            let _lock = lock_directory(directory)?;
            ensure_index(directory)?;
            read_run(directory, run_id)?
        } else {
            self.memory
                .lock()
                .map_err(|_| before("The check history could not be locked."))?
                .for_run(run_id)
        };
        receipts.sort_by(|a, b| {
            a.admitted_at
                .cmp(&b.admitted_at)
                .then(a.attempt.cmp(&b.attempt))
                .then(a.id.cmp(&b.id))
        });
        Ok(receipts)
    }

    /// Derived membership by command and certified inputs. It survives task and
    /// session changes; canonical receipt payloads remain the only authority.
    pub fn for_check(
        &self,
        request: &crate::ship::application::CheckRequest,
        source: &crate::ship::application::SourceObservation,
    ) -> Result<Vec<CheckReceipt>, ShipError> {
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
                if let Some(receipt) = read_receipt(directory, id)? {
                    if receipt_reuse_key(&receipt).as_deref() != Some(&key) {
                        return Err("Check input index does not match its receipt.".into());
                    }
                    records.push(receipt);
                }
            }
            records
        } else {
            self.memory
                .lock()
                .map_err(|_| before("The check history could not be locked."))?
                .records
                .values()
                .filter(|receipt| receipt_reuse_key(receipt).as_deref() == Some(&key))
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
    ) -> Result<ReceiptCompaction, ShipError> {
        let mut report = ReceiptCompaction::default();
        if let Some(directory) = &self.directory {
            report.storage = ReceiptStorage::File;
            let _lock = lock_directory(directory)?;
            ensure_index(directory)?;
            let mut retained = retained()?;
            let records = read_directory(directory)?;
            retain_reuse_origins(records.values(), &mut retained);
            for receipt in records.into_values() {
                if report.retain(&receipt, current_run, &retained) {
                    continue;
                }
                fs::remove_file(receipt_path(directory, &receipt.id)?).map_err(|e| {
                    let error = before(format!("The old check receipt could not be removed: {e}"));
                    if report.removed > 0 {
                        error.after_commit()
                    } else {
                        error
                    }
                })?;
                // The first successful unlink is already an effect, including
                // when its directory synchronization subsequently fails.
                sync_directory(directory).map_err(ShipError::after_commit)?;
                remove_membership(
                    &run_directory(directory, &receipt.target.run_id),
                    &receipt.id,
                )
                .map_err(ShipError::after_commit)?;
                if let Some(key) = receipt_reuse_key(&receipt) {
                    remove_membership(&directory.join("checks-v2").join(key), &receipt.id)?;
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
                if let Some(receipt) = memory.records.remove(&id) {
                    if let Some(ids) = memory.runs.get_mut(&receipt.target.run_id) {
                        ids.remove(&id);
                    }
                    report.removed += 1;
                }
            }
        }
        Ok(report)
    }
}

fn before(message: impl Into<String>) -> ShipError {
    WriteError::before_commit(message).into()
}

fn validate_id(id: &str) -> Result<(), ShipError> {
    if id.is_empty()
        || !id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
    {
        return Err("invalid receipt ID".into());
    }
    Ok(())
}

fn receipt_path(directory: &Path, id: &str) -> Result<PathBuf, ShipError> {
    validate_id(id)?;
    Ok(directory.join(format!("{id}.json")))
}

fn run_directory(directory: &Path, run: &str) -> PathBuf {
    directory
        .join("runs-v1")
        .join(hex::encode(Sha256::digest(run.as_bytes())))
}

fn lock_directory(directory: &Path) -> Result<File, ShipError> {
    fs::create_dir_all(directory).map_err(|e| before(format!("create receipt ledger: {e}")))?;
    let lock = OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(directory.join(".lock"))
        .map_err(|e| before(format!("open receipt ledger lock: {e}")))?;
    lock.lock()
        .map_err(|e| before(format!("lock receipt ledger: {e}")))?;
    Ok(lock)
}

fn read_receipt(directory: &Path, id: &str) -> Result<Option<CheckReceipt>, ShipError> {
    Ok(observe_receipt(directory, id)?.value)
}

fn observe_receipt(
    directory: &Path,
    id: &str,
) -> Result<Observed<Option<CheckReceipt>>, ShipError> {
    let read: Observed<Option<CheckReceipt>> = read_json(&receipt_path(directory, id)?)?;
    if let Some(receipt) = &read.value {
        if receipt.id != id {
            return Err(ReadError::new(
                ReadErrorKind::Identity,
                "The check receipt belongs to another attempt.",
            )
            .into());
        }
        let complete = receipt.outcome.is_some()
            && receipt.recorded_at.is_some()
            && receipt.duration_ms.is_some();
        let valid = match receipt.stage {
            ReceiptStage::Admitted => {
                receipt.outcome.is_none()
                    && receipt.recorded_at.is_none()
                    && receipt.settlement.is_none()
            }
            ReceiptStage::Recorded | ReceiptStage::Applying => {
                complete && receipt.settlement.is_none() && receipt.settled_at.is_none()
            }
            ReceiptStage::Settled => {
                complete && receipt.settlement.is_some() && receipt.settled_at.is_some()
            }
        };
        if !valid {
            return Err(
                ReadError::new(ReadErrorKind::Schema, "The check receipt is incomplete.").into(),
            );
        }
    }
    Ok(read)
}

fn read_directory(directory: &Path) -> Result<BTreeMap<String, CheckReceipt>, ShipError> {
    let mut records = BTreeMap::new();
    for entry in fs::read_dir(directory).map_err(|e| before(format!("read receipt ledger: {e}")))? {
        let path = entry
            .map_err(|e| before(format!("read receipt entry: {e}")))?
            .path();
        if path.extension().is_none_or(|e| e != "json") {
            continue;
        }
        let id = path
            .file_stem()
            .and_then(|s| s.to_str())
            .ok_or("invalid receipt filename")?;
        if let Some(receipt) = read_receipt(directory, id)? {
            records.insert(receipt.id.clone(), receipt);
        }
    }
    Ok(records)
}

fn ensure_index(directory: &Path) -> Result<(), ShipError> {
    let marker = directory.join("runs-v1").join("ready");
    match fs::read(&marker) {
        Ok(bytes) if bytes == b"1\n" => return Ok(()),
        Ok(_) => return Err("unsupported receipt index version".into()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => return Err(before(format!("read receipt index: {e}"))),
    }
    for receipt in read_directory(directory)?.into_values() {
        add_membership(directory, &receipt)?;
    }
    fs::create_dir_all(directory.join("runs-v1"))
        .map_err(|e| before(format!("create receipt index: {e}")))?;
    sync_directory(directory)?;
    // A crash before publication leaves this derived index rebuild retryable.
    publish_bytes(&directory.join("runs-v1"), "ready", b"1\n")
}

fn add_membership(directory: &Path, receipt: &CheckReceipt) -> Result<(), ShipError> {
    validate_id(&receipt.id)?;
    let bucket = run_directory(directory, &receipt.target.run_id);
    fs::create_dir_all(&bucket).map_err(|e| before(format!("create receipt run index: {e}")))?;
    let member = bucket.join(format!("{}.ref", receipt.id));
    // Idempotent also after a partial index build or interrupted admission.
    OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(member)
        .and_then(|file| file.sync_all())
        .map_err(|e| before(format!("persist receipt membership: {e}")))?;
    sync_directory(&bucket)?;
    sync_directory(&directory.join("runs-v1"))?;
    sync_directory(directory)
}

fn read_run(directory: &Path, run: &str) -> Result<Vec<CheckReceipt>, ShipError> {
    let mut receipts = Vec::new();
    let entries = match fs::read_dir(run_directory(directory, run)) {
        Ok(entries) => entries,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(receipts),
        Err(e) => return Err(before(format!("read receipt run index: {e}"))),
    };
    for entry in entries {
        let path = entry
            .map_err(|e| before(format!("read receipt membership: {e}")))?
            .path();
        if path.extension().is_none_or(|e| e != "ref") {
            continue;
        }
        let id = path
            .file_stem()
            .and_then(|s| s.to_str())
            .ok_or("invalid receipt membership")?;
        if let Some(receipt) = read_receipt(directory, id)? {
            if receipt.target.run_id != run {
                return Err("receipt run index mismatch".into());
            }
            receipts.push(receipt);
        }
    }
    Ok(receipts)
}

fn remove_membership(bucket: &Path, id: &str) -> Result<(), ShipError> {
    match fs::remove_file(bucket.join(format!("{id}.ref"))) {
        Ok(()) => sync_directory(bucket)?,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => return Err(before(format!("remove receipt membership: {e}"))),
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
        Err(e) => Err(before(format!("remove empty receipt run index: {e}"))),
    }
}

fn publish(directory: &Path, receipt: &CheckReceipt) -> Result<ReadSource, ShipError> {
    let existing = receipt_path(directory, &receipt.id)?.is_file();
    let run_member =
        run_directory(directory, &receipt.target.run_id).join(format!("{}.ref", receipt.id));
    if !existing || !run_member.is_file() {
        add_membership(directory, receipt)?;
    }
    add_reuse_membership(directory, receipt, existing)?;
    let stamp = write_locked(&receipt_path(directory, &receipt.id)?, receipt)?;
    Ok(ReadSource::File(stamp))
}

// The small derived index marker is also synced before its acknowledgement.
fn publish_bytes(directory: &Path, name: &str, bytes: &[u8]) -> Result<(), ShipError> {
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
fn sync_directory(directory: &Path) -> Result<(), ShipError> {
    File::open(directory)
        .and_then(|dir| dir.sync_all())
        .map_err(|e| before(format!("The check history could not be synchronized: {e}")))
}
#[cfg(not(unix))]
fn sync_directory(_directory: &Path) -> Result<(), ShipError> {
    Ok(())
}

fn reuse_key(
    gate: &str,
    check_type: &crate::ship::domain::check::CheckType,
    command: Option<&str>,
    source: Option<&crate::ship::application::SourceObservation>,
) -> Option<String> {
    let source = source?;
    if source.execution_digest.is_empty() {
        return None;
    }
    let key = serde_json::to_vec(&(
        gate,
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

fn receipt_reuse_key(receipt: &CheckReceipt) -> Option<String> {
    reuse_key(
        &receipt.gate,
        &receipt.check_type,
        receipt.command.as_deref(),
        receipt.source.as_ref(),
    )
}

fn add_reuse_membership(
    directory: &Path,
    receipt: &CheckReceipt,
    existing: bool,
) -> Result<(), ShipError> {
    let Some(key) = receipt_reuse_key(receipt) else {
        return Ok(());
    };
    let bucket = directory.join("checks-v2").join(key);
    let member = bucket.join(format!("{}.ref", receipt.id));
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

fn ensure_reuse_index(directory: &Path) -> Result<(), ShipError> {
    let marker = directory.join("checks-v2/ready");
    match fs::read(&marker) {
        Ok(bytes) if bytes == b"1\n" => return Ok(()),
        Ok(_) => return Err("Unsupported check input index version.".into()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(before(error.to_string())),
    }
    for receipt in read_directory(directory)?.into_values() {
        add_reuse_membership(directory, &receipt, false)?;
    }
    fs::create_dir_all(directory.join("checks-v2")).map_err(|error| before(error.to_string()))?;
    sync_directory(directory)?;
    publish_bytes(&directory.join("checks-v2"), "ready", b"1\n")
}

fn retain_reuse_origins<'a>(
    receipts: impl Iterator<Item = &'a CheckReceipt>,
    retained: &mut BTreeSet<String>,
) {
    for receipt in receipts {
        if let Some(crate::ship::application::CheckExecution::Reused { receipt_id }) =
            receipt.execution()
        {
            retained.insert(receipt_id.clone());
        }
    }
}
