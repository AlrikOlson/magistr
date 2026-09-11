use std::collections::BTreeSet;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::{Connection, OptionalExtension, params};

use crate::file_content::FileContent;

use super::journal_lifecycle::OpenedJournal;
use super::journal_queries::JournalQueries;
use super::{
    JournalAppendResult, JournalDbOptions, JournalError, JournalLogEntry, JournalLogQuery,
    JournalOp, JournalOpInit, UndoCandidateQuery,
};

/// Durable SQLite journal writer for the workspace owner.
pub struct JournalDb {
    db: Connection,
    cap_bytes: u64,
    pub lifecycle_notes: Vec<String>,
    pub quarantined_to: Option<PathBuf>,
}

impl JournalDb {
    /// Open under the composing application's sole workspace-writer admission.
    /// The caller must keep that admission for this object's entire lifetime;
    /// this adapter does not implement workspace ownership. Do not open another
    /// writer or independent checkpoint connection beside it. Use
    /// [`super::JournalReader`] for concurrent read views instead.
    pub fn open(options: JournalDbOptions) -> Result<Self, JournalError> {
        let opened = OpenedJournal::writer(&options)?;
        Ok(Self {
            db: opened.db,
            cap_bytes: options.cap_bytes,
            lifecycle_notes: opened.notes,
            quarantined_to: opened.quarantined_to,
        })
    }

    /// Commit an operation, undo links, ordered files, CAS recency and eviction
    /// together. Failed compression, hydration, or SQL rolls back the whole batch.
    pub fn append(&mut self, op: &JournalOpInit) -> Result<JournalAppendResult, JournalError> {
        validate_append(op)?;
        let at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|error| {
                JournalError::InvalidInput(format!("clock precedes Unix epoch: {error}"))
            })?
            .as_millis();
        let at = i64::try_from(at).map_err(|_| {
            JournalError::InvalidInput("timestamp exceeds SQLite integer range".to_owned())
        })?;
        let transaction = self.db.transaction()?;
        let is_undo = op.undo_of.as_ref().is_some_and(|seqs| !seqs.is_empty());
        transaction.execute(
            "INSERT INTO ops(at,tool,args_hash,session_id,is_undo) VALUES (?,?,?,?,?)",
            params![at, op.tool, op.args_hash, op.session_id, is_undo],
        )?;
        let seq = transaction.last_insert_rowid();
        for undone in op.undo_of.iter().flatten() {
            transaction.execute(
                "INSERT OR IGNORE INTO undone(op_seq,by_seq) VALUES (?,?)",
                params![undone, seq],
            )?;
        }
        let mut protect = BTreeSet::new();
        for (ord, file) in op.files.iter().enumerate() {
            let before = store_blob(&transaction, &file.before, seq)?;
            let after = store_blob(&transaction, &file.after, seq)?;
            transaction.execute("INSERT INTO op_files(op_seq,ord,kind,path,before_hash,after_hash) VALUES (?,?,?,?,?,?)", params![seq,ord as i64,file.kind.as_str(),file.path,before,after])?;
            protect.insert(before);
            protect.insert(after);
        }
        let notes = evict(&transaction, self.cap_bytes, &protect)?;
        let op = JournalQueries::new(&transaction).get(seq)?.ok_or_else(|| {
            JournalError::CorruptRecord("appended operation vanished within transaction".to_owned())
        })?;
        transaction.commit()?;
        Ok(JournalAppendResult { op, notes })
    }

    pub fn get(&self, seq: i64) -> Result<Option<JournalOp>, JournalError> {
        let snapshot = self.db.unchecked_transaction()?;
        JournalQueries::new(&snapshot).get(seq)
    }

    /// Ascending append order; refuses more than 1000 ops without truncation.
    pub fn list(&self, since_seq: i64) -> Result<Vec<JournalOp>, JournalError> {
        let snapshot = self.db.unchecked_transaction()?;
        JournalQueries::new(&snapshot).list(since_seq)
    }

    pub fn recent(&self, query: &JournalLogQuery) -> Result<Vec<JournalLogEntry>, JournalError> {
        let snapshot = self.db.unchecked_transaction()?;
        JournalQueries::new(&snapshot).recent(query)
    }

    pub fn undo_candidates(
        &self,
        query: &UndoCandidateQuery,
    ) -> Result<Vec<JournalOp>, JournalError> {
        let snapshot = self.db.unchecked_transaction()?;
        JournalQueries::new(&snapshot).candidates(query)
    }
}

fn validate_append(op: &JournalOpInit) -> Result<(), JournalError> {
    if op.files.len() > 1000
        || op
            .undo_of
            .as_ref()
            .is_some_and(|seqs| seqs.len() > 1000 || seqs.iter().any(|seq| *seq < 1))
    {
        return Err(JournalError::InvalidInput(
            "append exceeds 1000 files/undo links or contains a nonpositive undo sequence"
                .to_owned(),
        ));
    }
    let mut bytes = op
        .tool
        .len()
        .saturating_add(op.args_hash.len())
        .saturating_add(op.session_id.len());
    for value in [&op.tool, &op.args_hash, &op.session_id]
        .into_iter()
        .chain(op.files.iter().map(|file| &file.path))
    {
        if value.len() > 1024 * 1024 {
            return Err(JournalError::InvalidInput(
                "append metadata exceeds 1 MiB per field".to_owned(),
            ));
        }
    }
    for file in &op.files {
        bytes = bytes
            .saturating_add(file.path.len())
            .saturating_add(134)
            .saturating_add(file.before.as_bytes().len())
            .saturating_add(file.after.as_bytes().len());
    }
    if bytes > 64 * 1024 * 1024 {
        return Err(JournalError::InvalidInput(
            "append exceeds 64 MiB hydrated byte budget".to_owned(),
        ));
    }
    Ok(())
}

fn store_blob(db: &Connection, content: &FileContent, seq: i64) -> Result<String, JournalError> {
    let hash = content.content_hash();
    let exists: bool = db.query_row(
        "SELECT EXISTS(SELECT 1 FROM blobs WHERE hash=?)",
        [&hash],
        |row| row.get(0),
    )?;
    if exists {
        // Retained compressed bytes are part of the compatibility contract.
        // Reusing content only advances recency; no codec work or blob rewrite.
        db.execute(
            "UPDATE blobs SET last_ref_seq=? WHERE hash=? AND last_ref_seq<?",
            params![seq, hash, seq],
        )?;
    } else {
        let compressed = zstd::stream::encode_all(content.as_bytes(), 0)?;
        db.execute(
            "INSERT INTO blobs(hash,zstd,bytes,last_ref_seq) VALUES (?,?,?,?)",
            params![hash, compressed, compressed.len() as i64, seq],
        )?;
    }
    Ok(hash)
}

fn evict(
    db: &Connection,
    cap: u64,
    protect: &BTreeSet<String>,
) -> Result<Vec<String>, JournalError> {
    let invalid_sizes: bool = db.query_row("SELECT EXISTS(SELECT 1 FROM blobs WHERE bytes<0 OR bytes!=length(zstd) OR typeof(bytes)!='integer' OR typeof(zstd)!='blob')", [], |row| row.get(0))?;
    if invalid_sizes {
        return Err(JournalError::CorruptRecord(
            "inconsistent retained compressed-byte accounting".to_owned(),
        ));
    }
    let total: i64 = db.query_row("SELECT COALESCE(SUM(bytes),0) FROM blobs", [], |row| {
        row.get(0)
    })?;
    if total < 0 {
        return Err(JournalError::CorruptRecord(
            "negative retained compressed-byte total".to_owned(),
        ));
    }
    let mut remaining = total as u64;
    let mut evicted = 0;
    let mut cursor: Option<(i64, String)> = None;
    while remaining > cap {
        let row: Option<(String, i64, i64)> = db.query_row("SELECT hash,bytes,last_ref_seq FROM blobs WHERE (?1 IS NULL OR last_ref_seq>?1 OR (last_ref_seq=?1 AND hash>?2)) ORDER BY last_ref_seq,hash LIMIT 1", params![cursor.as_ref().map(|cursor| cursor.0),cursor.as_ref().map(|cursor| cursor.1.as_str())], |row| Ok((row.get(0)?,row.get(1)?,row.get(2)?))).optional()?;
        let Some((hash, bytes, last_ref)) = row else {
            break;
        };
        if bytes < 0 {
            return Err(JournalError::CorruptRecord(
                "negative compressed blob size".to_owned(),
            ));
        }
        if !protect.contains(&hash) {
            db.execute("DELETE FROM blobs WHERE hash=?", [&hash])?;
            remaining = remaining.checked_sub(bytes as u64).ok_or_else(|| {
                JournalError::CorruptRecord("invalid compressed-byte accounting".to_owned())
            })?;
            evicted += 1;
        }
        cursor = Some((last_ref, hash));
    }
    let mut notes = Vec::new();
    if evicted > 0 {
        notes.push(format!("journal: evicted {evicted} oldest blob(s) ({} compressed bytes) to stay under the {cap}-byte cap; the ops that referenced them are no longer undoable", total as u64 - remaining));
    }
    if remaining > cap {
        notes.push(format!("journal: protected newest operation retains {remaining} compressed bytes, exceeding the {cap}-byte cap; its own blobs were preserved"));
    }
    Ok(notes)
}

#[cfg(test)]
#[path = "journal_db.tests.rs"]
mod tests;
