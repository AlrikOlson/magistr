use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::{MutationStatus, ReceiptFile, workspace_identity::frame};
use crate::journal::{
    JournalDb, JournalError, JournalFileKind, JournalOp, JournalReader, JournalWriteOutcome,
};

/// The truthful application receipt of one journaled mutation, read from the
/// durable intent the write engine resolved after its swaps — never from
/// the journal sequence, which is allocated before any file changes. A
/// revision exists only for an applied mutation; a pending intent after an
/// interruption, a failed compensation or legacy history is indeterminate.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct MutationReceipt {
    pub journal_seq: i64,
    /// The owner generation the reader held, if any; the journal itself
    /// does not record which incarnation applied an op.
    pub generation: Option<u64>,
    pub tool: String,
    pub session_id: String,
    pub status: MutationStatus,
    pub files: Vec<ReceiptFile>,
    /// SHA-256 over the applied files' paths, kinds and after-hashes;
    /// `None` unless the status is applied.
    pub revision: Option<String>,
    pub notes: Vec<String>,
}

impl MutationReceipt {
    /// Read op `seq` through the sole writer; `None` when no such op exists.
    pub fn from_journal(
        journal: &JournalDb,
        seq: i64,
        generation: Option<u64>,
    ) -> Result<Option<Self>, JournalError> {
        let Some(op) = journal.get(seq)? else {
            return Ok(None);
        };
        Ok(Some(Self::recover(
            &op,
            journal.write_outcome(seq)?,
            generation,
        )))
    }

    /// Read op `seq` through a lock-free reader; `None` when absent.
    pub fn from_reader(
        reader: &JournalReader,
        seq: i64,
        generation: Option<u64>,
    ) -> Result<Option<Self>, JournalError> {
        let Some(op) = reader.get(seq)? else {
            return Ok(None);
        };
        Ok(Some(Self::recover(
            &op,
            reader.write_outcome(seq)?,
            generation,
        )))
    }

    /// The receipt for an op given the durable outcome the journal answered.
    #[must_use]
    pub fn recover(
        op: &JournalOp,
        outcome: Option<JournalWriteOutcome>,
        generation: Option<u64>,
    ) -> Self {
        let (status, note) = match outcome {
            Some(JournalWriteOutcome::Applied) => (MutationStatus::Applied, None),
            Some(JournalWriteOutcome::RolledBack) => (
                MutationStatus::Failed,
                Some(
                    "a swap failed and every attempted file was restored; nothing of this mutation remains",
                ),
            ),
            Some(JournalWriteOutcome::Pending) => (
                MutationStatus::Indeterminate,
                Some(
                    "the intent was never resolved: the owner was interrupted between journaling and applying; inspect the live files",
                ),
            ),
            Some(JournalWriteOutcome::Indeterminate) => (
                MutationStatus::Indeterminate,
                Some(
                    "compensation or outcome persistence failed after a partial application; inspect the live files",
                ),
            ),
            None => (
                MutationStatus::Indeterminate,
                Some(
                    "legacy journal history carries no native intent; application was never certified",
                ),
            ),
        };
        let files: Vec<ReceiptFile> = op
            .files
            .iter()
            .map(|file| ReceiptFile {
                path: file.path.clone(),
                kind: match file.kind {
                    JournalFileKind::Create => "create",
                    JournalFileKind::Write => "write",
                    JournalFileKind::Delete => "delete",
                }
                .into(),
                before_hash: file.before_hash.clone(),
                after_hash: file.after_hash.clone(),
            })
            .collect();
        let revision = (status == MutationStatus::Applied).then(|| {
            let mut digest = Sha256::new();
            frame(&mut digest, b"magistr-mutation-receipt/1");
            frame(&mut digest, &op.seq.to_le_bytes());
            for file in &files {
                frame(&mut digest, file.path.as_bytes());
                frame(&mut digest, file.kind.as_bytes());
                frame(&mut digest, file.after_hash.as_bytes());
            }
            format!("{:x}", digest.finalize())
        });
        Self {
            journal_seq: op.seq,
            generation,
            tool: op.tool.clone(),
            session_id: op.session_id.clone(),
            status,
            files,
            revision,
            notes: note.map(str::to_owned).into_iter().collect(),
        }
    }
}

#[cfg(test)]
#[path = "mutation_receipt.tests.rs"]
mod tests;
