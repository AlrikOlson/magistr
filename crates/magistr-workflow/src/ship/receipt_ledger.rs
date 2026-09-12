//! Durable check receipts, one file per receipt, independent of the cycle.
//!
//! The ship session record is cycle-shaped and is wiped when a fresh
//! objective follows a finished one; receipts are not. They live beside it
//! under `ship/receipts/<project>/<id>.json`, published by temp → fsync →
//! rename → fsync-directory so a reader never sees a torn record, and are
//! read without any lock. The ledger's own lock covers only attempt
//! allocation and the rewrite at settlement — never a command, never a
//! source observation, and never the engine's lock.

use std::collections::BTreeMap;
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use crate::ship::application::TaskAdmission;
use crate::ship::domain::check_receipt::CheckReceipt;

/// The receipt store for one project: on disk under the store's `ship`
/// partition, or in memory when persistence is off.
#[derive(Debug, Clone)]
pub struct ReceiptLedger {
    directory: Option<PathBuf>,
    memory: Arc<Mutex<BTreeMap<String, CheckReceipt>>>,
}

impl ReceiptLedger {
    /// A ledger that forgets at exit.
    #[must_use]
    pub fn in_memory() -> Self {
        Self {
            directory: None,
            memory: Arc::new(Mutex::new(BTreeMap::new())),
        }
    }

    /// A ledger under `directory` (created on first write).
    #[must_use]
    pub fn on_disk(directory: impl Into<PathBuf>) -> Self {
        Self {
            directory: Some(directory.into()),
            memory: Arc::new(Mutex::new(BTreeMap::new())),
        }
    }

    /// Where receipts are written, when they are.
    #[must_use]
    pub fn directory(&self) -> Option<&Path> {
        self.directory.as_deref()
    }

    /// Allocate the next attempt for `target` and `gate`, build the receipt
    /// with it and publish it. Attempt numbers are per run, task instance
    /// and gate, so a reused task name in a later run starts again at one.
    ///
    /// # Errors
    /// The store cannot be locked, read or written.
    pub fn admit(
        &self,
        target: &TaskAdmission,
        gate: &str,
        build: impl FnOnce(u64) -> CheckReceipt,
    ) -> Result<CheckReceipt, String> {
        self.update(|records| {
            let attempt = records
                .values()
                .filter(|receipt| {
                    receipt.target.run_id == target.run_id
                        && receipt.target.task_instance == target.task_instance
                        && receipt.gate == gate
                })
                .map(|receipt| receipt.attempt)
                .max()
                .unwrap_or(0)
                .checked_add(1)
                .ok_or_else(|| "receipt attempt counter exhausted".to_owned())?;
            let receipt = build(attempt);
            records.insert(receipt.id.clone(), receipt.clone());
            Ok(receipt)
        })
    }

    /// Rewrite the receipt `id` through `settle`; a receipt this ledger
    /// never saw (admitted through another store) is created from
    /// `fallback` first so a late result always has a record.
    ///
    /// # Errors
    /// The store cannot be locked, read or written.
    pub fn settle(
        &self,
        id: &str,
        fallback: impl FnOnce() -> CheckReceipt,
        settle: impl FnOnce(&mut CheckReceipt),
    ) -> Result<CheckReceipt, String> {
        self.update(|records| {
            let mut receipt = records.remove(id).unwrap_or_else(fallback);
            settle(&mut receipt);
            records.insert(receipt.id.clone(), receipt.clone());
            Ok(receipt)
        })
    }

    /// One receipt by id.
    ///
    /// # Errors
    /// The store cannot be read.
    pub fn get(&self, id: &str) -> Result<Option<CheckReceipt>, String> {
        Ok(self.read()?.remove(id))
    }

    /// Every receipt admitted in `run_id`, oldest admission first.
    ///
    /// # Errors
    /// The store cannot be read.
    pub fn for_run(&self, run_id: &str) -> Result<Vec<CheckReceipt>, String> {
        let mut receipts: Vec<CheckReceipt> = self
            .read()?
            .into_values()
            .filter(|receipt| receipt.target.run_id == run_id)
            .collect();
        receipts.sort_by(|a, b| {
            a.admitted_at
                .cmp(&b.admitted_at)
                .then(a.attempt.cmp(&b.attempt))
                .then(a.id.cmp(&b.id))
        });
        Ok(receipts)
    }

    fn read(&self) -> Result<BTreeMap<String, CheckReceipt>, String> {
        match &self.directory {
            None => self
                .memory
                .lock()
                .map(|records| records.clone())
                .map_err(|_| "receipt ledger lock poisoned".to_owned()),
            Some(directory) => read_directory(directory),
        }
    }

    fn update(
        &self,
        operation: impl FnOnce(&mut BTreeMap<String, CheckReceipt>) -> Result<CheckReceipt, String>,
    ) -> Result<CheckReceipt, String> {
        let Some(directory) = &self.directory else {
            let mut records = self
                .memory
                .lock()
                .map_err(|_| "receipt ledger lock poisoned".to_owned())?;
            return operation(&mut records);
        };
        let _lock = lock_directory(directory)?;
        let mut records = read_directory(directory)?;
        let receipt = operation(&mut records)?;
        publish(directory, &receipt)?;
        Ok(receipt)
    }
}

fn lock_directory(directory: &Path) -> Result<File, String> {
    fs::create_dir_all(directory).map_err(|error| format!("create receipt ledger: {error}"))?;
    let lock = OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(directory.join(".lock"))
        .map_err(|error| format!("open receipt ledger lock: {error}"))?;
    lock.lock()
        .map_err(|error| format!("lock receipt ledger: {error}"))?;
    Ok(lock)
}

fn read_directory(directory: &Path) -> Result<BTreeMap<String, CheckReceipt>, String> {
    let mut records = BTreeMap::new();
    let entries = match fs::read_dir(directory) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(records),
        Err(error) => return Err(format!("read receipt ledger: {error}")),
    };
    for entry in entries {
        let path = entry
            .map_err(|error| format!("read receipt entry: {error}"))?
            .path();
        if path.extension().is_none_or(|extension| extension != "json") {
            continue;
        }
        let bytes =
            fs::read(&path).map_err(|error| format!("read receipt {}: {error}", path.display()))?;
        let receipt: CheckReceipt = serde_json::from_slice(&bytes)
            .map_err(|error| format!("decode receipt {}: {error}", path.display()))?;
        if path.file_stem().and_then(|stem| stem.to_str()) != Some(receipt.id.as_str()) {
            return Err(format!("receipt identity mismatch: {}", path.display()));
        }
        records.insert(receipt.id.clone(), receipt);
    }
    Ok(records)
}

fn publish(directory: &Path, receipt: &CheckReceipt) -> Result<(), String> {
    let bytes =
        serde_json::to_vec_pretty(receipt).map_err(|error| format!("encode receipt: {error}"))?;
    let temporary = directory.join(format!(".{}-{}.tmp", receipt.id, std::process::id()));
    let target = directory.join(format!("{}.json", receipt.id));
    let mut file = OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .open(&temporary)
        .map_err(|error| format!("create receipt temporary: {error}"))?;
    file.write_all(&bytes)
        .and_then(|()| file.sync_all())
        .map_err(|error| format!("persist receipt: {error}"))?;
    fs::rename(&temporary, &target).map_err(|error| format!("publish receipt: {error}"))?;
    sync_directory(directory)
}

/// Make the rename durable. Directory fsync is a Unix notion; Windows cannot
/// open a directory as a plain file and NTFS journals the rename itself.
#[cfg(unix)]
fn sync_directory(directory: &Path) -> Result<(), String> {
    File::open(directory)
        .and_then(|dir| dir.sync_all())
        .map_err(|error| format!("sync receipt ledger: {error}"))
}

#[cfg(not(unix))]
fn sync_directory(_directory: &Path) -> Result<(), String> {
    Ok(())
}
