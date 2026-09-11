use std::path::Path;

use rusqlite::Connection;

use super::journal_lifecycle::OpenedJournal;
use super::journal_queries::JournalQueries;
use super::{JournalError, JournalLogEntry, JournalLogQuery, JournalOp, UndoCandidateQuery};

/// Read-only retained-format journal connection.
pub struct JournalReader {
    db: Connection,
}

impl JournalReader {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, JournalError> {
        Ok(Self {
            db: OpenedJournal::reader(path.as_ref())?,
        })
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
