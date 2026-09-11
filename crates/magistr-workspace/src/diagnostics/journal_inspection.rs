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
    /// The rows themselves, or the typed refusal; `None` when this
    /// workspace has no journal yet. Hosts that shape their own envelope
    /// (the `journal` CLI verb) read here; `list` wraps the same rows in
    /// the budgeted envelope.
    pub fn rows(
        state_dir: &Path,
        query: &JournalLogQuery,
        options: &JournalViewOptions,
    ) -> Result<Option<Vec<JournalRow>>, WorkspaceError> {
        let path = state_dir.join("journal.db");
        if !path.exists() {
            return Ok(None);
        }
        JournalReader::open(&path)
            .and_then(|reader| reader.recent(query))
            .map(|entries| Some(JournalRow::render(&entries, options)))
            .map_err(Self::map)
    }

    pub fn list(
        state_dir: &Path,
        query: &JournalLogQuery,
        options: &JournalViewOptions,
        requested: u64,
    ) -> WorkspaceResponse<JournalRow> {
        match Self::rows(state_dir, query, options) {
            Ok(Some(rows)) => Self::respond(rows, requested, Vec::new()),
            Ok(None) => Self::respond(
                Vec::new(),
                requested,
                vec!["no journal: this workspace has not been written through the pipeline".into()],
            ),
            Err(error) => budget::failure(error),
        }
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
