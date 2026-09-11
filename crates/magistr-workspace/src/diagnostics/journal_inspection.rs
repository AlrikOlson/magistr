use std::path::Path;

use super::{JournalRow, JournalViewOptions};
use crate::{
    contracts::{ErrorKind, Freshness, WorkspaceError, WorkspaceResponse},
    journal::{JournalError, JournalLogQuery, JournalReader},
    query::budget,
};

/// The journal as rows, read beside a live owner: a read-only reader
/// connection, no lock taken, nothing under the state directory rewritten.
/// A workspace that never wrote answers no rows; a journal written by a
/// newer binary fails closed as UNSUPPORTED_VERSION with the file untouched.
pub struct JournalInspection;

impl JournalInspection {
    pub fn list(
        state_dir: &Path,
        query: &JournalLogQuery,
        options: &JournalViewOptions,
        requested: u64,
    ) -> WorkspaceResponse<JournalRow> {
        let path = state_dir.join("journal.db");
        if !path.exists() {
            return Self::respond(
                Vec::new(),
                requested,
                vec!["no journal: this workspace has not been written through the pipeline".into()],
            );
        }
        let entries = match JournalReader::open(&path).and_then(|reader| reader.recent(query)) {
            Ok(entries) => entries,
            Err(error) => return budget::failure(Self::map(error)),
        };
        Self::respond(JournalRow::render(&entries, options), requested, Vec::new())
    }

    fn respond(
        rows: Vec<JournalRow>,
        requested: u64,
        notes: Vec<String>,
    ) -> WorkspaceResponse<JournalRow> {
        budget::success(
            rows,
            requested,
            Freshness::Warm,
            notes,
            |row| format!("seq:{}", row.seq),
            vec![
                "lower limit or pass before".into(),
                "diff:false".into(),
                "raise budget".into(),
            ],
        )
    }

    fn map(error: JournalError) -> WorkspaceError {
        match error {
            JournalError::UnsupportedVersion(detail) => WorkspaceError::new(
                ErrorKind::UnsupportedVersion,
                format!(
                    "the journal cannot be read by this binary: {detail}; upgrade magistr — durable data was left untouched"
                ),
            ),
            JournalError::InvalidInput(detail) => WorkspaceError::new(
                ErrorKind::PatternInvalid,
                format!("journal query: {detail}"),
            ),
            other => WorkspaceError::new(
                ErrorKind::EngineUnavailable,
                format!("the journal cannot be read: {other}"),
            ),
        }
    }
}

#[cfg(test)]
#[path = "journal_inspection.tests.rs"]
mod tests;
