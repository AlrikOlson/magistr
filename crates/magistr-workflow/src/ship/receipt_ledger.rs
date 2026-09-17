//! Durable check receipts with a derived, versioned per-run membership index.
//!
//! Canonical payloads remain <id>.json. Membership is synced BEFORE payload
//! publication, so an interrupted admission cannot hide an acknowledged receipt.
//! A dangling membership names an admission that never published (or a pruned
//! receipt). All ledger operations serialize on .lock; no command or observation
//! runs under it. Legacy stores build their index once, without altering receipts.

#[cfg(test)]
#[path = "receipt_ledger.tests.rs"]
mod tests;

use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use super::receipt_compaction::ReceiptCompaction;
use crate::ship::application::TaskAdmission;
use crate::ship::domain::check_receipt::CheckReceipt;

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

/// The receipt store for one project. Reads are bounded by the requested run
/// (or exact ID), after a one-time legacy index build. Pruning is explicit.
#[derive(Debug, Clone)]
pub struct ReceiptLedger {
    directory: Option<PathBuf>,
    memory: Arc<Mutex<Memory>>,
}

impl ReceiptLedger {
    #[must_use]
    pub fn in_memory() -> Self {
        Self {
            directory: None,
            memory: Arc::new(Mutex::new(Memory::default())),
        }
    }

    #[must_use]
    pub fn on_disk(directory: impl Into<PathBuf>) -> Self {
        Self {
            directory: Some(directory.into()),
            memory: Arc::new(Mutex::new(Memory::default())),
        }
    }

    #[must_use]
    pub fn directory(&self) -> Option<&Path> {
        self.directory.as_deref()
    }

    /// Allocate an attempt from only this run and publish before returning.
    pub fn admit(
        &self,
        target: &TaskAdmission,
        gate: &str,
        build: impl FnOnce(u64) -> CheckReceipt,
    ) -> Result<CheckReceipt, String> {
        let make = |records: Vec<CheckReceipt>| -> Result<CheckReceipt, String> {
            let attempt = records
                .iter()
                .filter(|r| r.target.task_instance == target.task_instance && r.gate == gate)
                .map(|r| r.attempt)
                .max()
                .unwrap_or(0)
                .checked_add(1)
                .ok_or_else(|| "receipt attempt counter exhausted".to_owned())?;
            let receipt = build(attempt);
            validate_id(&receipt.id)?;
            if receipt.target != *target || receipt.gate != gate || receipt.attempt != attempt {
                return Err("receipt does not match its admission".into());
            }
            Ok(receipt)
        };
        if let Some(directory) = &self.directory {
            let _lock = lock_directory(directory)?;
            ensure_index(directory)?;
            let receipt = make(read_run(directory, &target.run_id)?)?;
            if read_receipt(directory, &receipt.id)?.is_some() {
                return Err("receipt ID is already admitted".into());
            }
            publish(directory, &receipt)?;
            Ok(receipt)
        } else {
            let mut memory = self
                .memory
                .lock()
                .map_err(|_| "receipt ledger lock poisoned")?;
            let receipt = make(memory.for_run(&target.run_id))?;
            if memory.records.contains_key(&receipt.id) {
                return Err("receipt ID is already admitted".into());
            }
            memory.insert(receipt.clone());
            Ok(receipt)
        }
    }

    /// Settle by exact ID, retaining the original admission and attempt.
    /// Missing admissions (from another store) retain the existing fallback rule.
    pub fn settle(
        &self,
        id: &str,
        fallback: impl FnOnce() -> CheckReceipt,
        settle: impl FnOnce(&mut CheckReceipt),
    ) -> Result<CheckReceipt, String> {
        validate_id(id)?;
        let finish = |found: Option<CheckReceipt>| {
            let mut receipt = found.unwrap_or_else(fallback);
            let target = receipt.target.clone();
            settle(&mut receipt);
            if receipt.id != id || receipt.target != target {
                return Err("settlement changed the receipt identity".to_owned());
            }
            Ok(receipt)
        };
        if let Some(directory) = &self.directory {
            let _lock = lock_directory(directory)?;
            ensure_index(directory)?;
            let receipt = finish(read_receipt(directory, id)?)?;
            publish(directory, &receipt)?;
            Ok(receipt)
        } else {
            let mut memory = self
                .memory
                .lock()
                .map_err(|_| "receipt ledger lock poisoned")?;
            let receipt = finish(memory.records.get(id).cloned())?;
            memory.insert(receipt.clone());
            Ok(receipt)
        }
    }

    /// Read one canonical receipt, without decoding any other receipt.
    pub fn get(&self, id: &str) -> Result<Option<CheckReceipt>, String> {
        validate_id(id)?;
        if let Some(directory) = &self.directory {
            let _lock = lock_directory(directory)?;
            read_receipt(directory, id)
        } else {
            Ok(self
                .memory
                .lock()
                .map_err(|_| "receipt ledger lock poisoned")?
                .records
                .get(id)
                .cloned())
        }
    }

    /// Read only this run's membership and payloads, oldest admission first.
    pub fn for_run(&self, run_id: &str) -> Result<Vec<CheckReceipt>, String> {
        let mut receipts = if let Some(directory) = &self.directory {
            let _lock = lock_directory(directory)?;
            ensure_index(directory)?;
            read_run(directory, run_id)?
        } else {
            self.memory
                .lock()
                .map_err(|_| "receipt ledger lock poisoned")?
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

    /// Remove settled history outside current_run. The caller must hold the
    /// persisted cycle lock while selecting current_run and through this call.
    /// Every current-run receipt and incomplete settlement is retained.
    pub(crate) fn compact(
        &self,
        current_run: &str,
        retained: impl FnOnce() -> Result<BTreeSet<String>, String>,
    ) -> Result<ReceiptCompaction, String> {
        let mut report = ReceiptCompaction::default();
        if let Some(directory) = &self.directory {
            let _lock = lock_directory(directory)?;
            ensure_index(directory)?;
            let retained = retained()?;
            // Decode before deleting anything: corrupt history refuses compaction.
            for receipt in read_directory(directory)?.into_values() {
                if report.retain(&receipt, current_run, &retained) {
                    continue;
                }
                fs::remove_file(receipt_path(directory, &receipt.id)?)
                    .map_err(|e| format!("remove settled receipt: {e}"))?;
                sync_directory(directory)?;
                let bucket = run_directory(directory, &receipt.target.run_id);
                remove_membership(&bucket, &receipt.id)?;
                report.removed += 1;
            }
        } else {
            let mut memory = self
                .memory
                .lock()
                .map_err(|_| "receipt ledger lock poisoned")?;
            let retained = retained()?;
            let removed: Vec<_> = memory
                .records
                .values()
                .filter(|r| !report.retain(r, current_run, &retained))
                .map(|r| (r.id.clone(), r.target.run_id.clone()))
                .collect();
            for (id, run) in removed {
                memory.records.remove(&id);
                if let Some(ids) = memory.runs.get_mut(&run) {
                    ids.remove(&id);
                    if ids.is_empty() {
                        memory.runs.remove(&run);
                    }
                }
                report.removed += 1;
            }
        }
        Ok(report)
    }
}

fn validate_id(id: &str) -> Result<(), String> {
    if id.is_empty()
        || !id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
    {
        return Err("invalid receipt ID".into());
    }
    Ok(())
}

fn receipt_path(directory: &Path, id: &str) -> Result<PathBuf, String> {
    validate_id(id)?;
    Ok(directory.join(format!("{id}.json")))
}

fn run_directory(directory: &Path, run: &str) -> PathBuf {
    directory
        .join("runs-v1")
        .join(format!("{:x}", Sha256::digest(run.as_bytes())))
}

fn lock_directory(directory: &Path) -> Result<File, String> {
    fs::create_dir_all(directory).map_err(|e| format!("create receipt ledger: {e}"))?;
    let lock = OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(directory.join(".lock"))
        .map_err(|e| format!("open receipt ledger lock: {e}"))?;
    lock.lock()
        .map_err(|e| format!("lock receipt ledger: {e}"))?;
    Ok(lock)
}

fn read_receipt(directory: &Path, id: &str) -> Result<Option<CheckReceipt>, String> {
    let path = receipt_path(directory, id)?;
    let bytes = match fs::read(&path) {
        Ok(bytes) => bytes,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(format!("read receipt {}: {e}", path.display())),
    };
    let receipt: CheckReceipt = serde_json::from_slice(&bytes)
        .map_err(|e| format!("decode receipt {}: {e}", path.display()))?;
    if receipt.id != id {
        return Err(format!("receipt identity mismatch: {}", path.display()));
    }
    Ok(Some(receipt))
}

fn read_directory(directory: &Path) -> Result<BTreeMap<String, CheckReceipt>, String> {
    let mut records = BTreeMap::new();
    for entry in fs::read_dir(directory).map_err(|e| format!("read receipt ledger: {e}"))? {
        let path = entry
            .map_err(|e| format!("read receipt entry: {e}"))?
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

fn ensure_index(directory: &Path) -> Result<(), String> {
    let marker = directory.join("runs-v1").join("ready");
    match fs::read(&marker) {
        Ok(bytes) if bytes == b"1\n" => return Ok(()),
        Ok(_) => return Err("unsupported receipt index version".into()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => return Err(format!("read receipt index: {e}")),
    }
    for receipt in read_directory(directory)?.into_values() {
        add_membership(directory, &receipt)?;
    }
    fs::create_dir_all(directory.join("runs-v1"))
        .map_err(|e| format!("create receipt index: {e}"))?;
    sync_directory(directory)?;
    // A crash before this atomic publication leaves the migration retryable.
    publish_bytes(&directory.join("runs-v1"), "ready", b"1\n")
}

fn add_membership(directory: &Path, receipt: &CheckReceipt) -> Result<(), String> {
    validate_id(&receipt.id)?;
    let bucket = run_directory(directory, &receipt.target.run_id);
    fs::create_dir_all(&bucket).map_err(|e| format!("create receipt run index: {e}"))?;
    let member = bucket.join(format!("{}.ref", receipt.id));
    // Idempotent also after a partial index build or interrupted admission.
    OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(member)
        .and_then(|file| file.sync_all())
        .map_err(|e| format!("persist receipt membership: {e}"))?;
    sync_directory(&bucket)?;
    sync_directory(&directory.join("runs-v1"))?;
    sync_directory(directory)
}

fn read_run(directory: &Path, run: &str) -> Result<Vec<CheckReceipt>, String> {
    let mut receipts = Vec::new();
    let entries = match fs::read_dir(run_directory(directory, run)) {
        Ok(entries) => entries,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(receipts),
        Err(e) => return Err(format!("read receipt run index: {e}")),
    };
    for entry in entries {
        let path = entry
            .map_err(|e| format!("read receipt membership: {e}"))?
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

fn remove_membership(bucket: &Path, id: &str) -> Result<(), String> {
    match fs::remove_file(bucket.join(format!("{id}.ref"))) {
        Ok(()) => sync_directory(bucket)?,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => return Err(format!("remove receipt membership: {e}")),
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
        Err(e) => Err(format!("remove empty receipt run index: {e}")),
    }
}

fn publish(directory: &Path, receipt: &CheckReceipt) -> Result<(), String> {
    let bytes = serde_json::to_vec_pretty(receipt).map_err(|e| format!("encode receipt: {e}"))?;
    add_membership(directory, receipt)?;
    publish_bytes(directory, &format!("{}.json", receipt.id), &bytes)
}

fn publish_bytes(directory: &Path, name: &str, bytes: &[u8]) -> Result<(), String> {
    let temporary = directory.join(format!(".{name}-{}.tmp", std::process::id()));
    let mut file = OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .open(&temporary)
        .map_err(|e| format!("create receipt temporary: {e}"))?;
    file.write_all(bytes)
        .and_then(|()| file.sync_all())
        .map_err(|e| format!("persist receipt: {e}"))?;
    fs::rename(&temporary, directory.join(name)).map_err(|e| format!("publish receipt: {e}"))?;
    sync_directory(directory)
}

#[cfg(unix)]
fn sync_directory(directory: &Path) -> Result<(), String> {
    File::open(directory)
        .and_then(|dir| dir.sync_all())
        .map_err(|e| format!("sync receipt ledger: {e}"))
}

#[cfg(not(unix))]
fn sync_directory(_directory: &Path) -> Result<(), String> {
    Ok(())
}
