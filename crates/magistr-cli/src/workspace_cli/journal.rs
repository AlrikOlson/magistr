//! `magistr workspace journal`: the write journal, read beside a live
//! server — read-only, no lock, no PRAGMA, no migration — as the rows
//! `undo action:"list"` returns, rendered as text or as one envelope.

use std::path::Path;

use color_eyre::eyre;
use magistr_workspace::{
    contracts::{
        Budget, EnvelopeVersion, ErrorKind, Freshness, ResultSet, WorkspaceError, WorkspaceResponse,
    },
    diagnostics::{JournalInspection, JournalRow, JournalViewOptions},
    journal::JournalLogQuery,
    response::ResponseAdapter,
    service::{JournalListInput, WorkspaceOpenError},
};

use super::{JournalView, admitted_root, exit::EXIT_USAGE, fail};

/// The retained wire estimator: JavaScript string length over four.
const CHARS_PER_TOKEN: usize = 4;

/// List the journal of the workspace at `dir` with the retained exit codes.
pub fn run_journal(dir: &Path, view: &JournalView) -> eyre::Result<()> {
    if !(1..=JournalListInput::LIMIT_MAX).contains(&view.limit) {
        fail(
            EXIT_USAGE,
            &format!(
                "--limit expects an integer in [1, {}]",
                JournalListInput::LIMIT_MAX
            ),
        );
    }
    let root = admitted_root(dir);
    let state_dir = root.join(".magistr/workspace");
    let query = JournalLogQuery {
        session_id: view.session.clone(),
        limit: view.limit,
        since_seq: view.since.map(seq),
        before_seq: view.before.map(seq),
    };
    let options = JournalViewOptions {
        diff: view.diff,
        ..JournalViewOptions::default()
    };
    let (rows, notes) = match JournalInspection::rows(&state_dir, &query, &options) {
        Ok(Some(rows)) => (rows, Vec::new()),
        Ok(None) => (
            Vec::new(),
            vec![format!(
                "no journal yet at {} — nothing has been written through this workspace",
                state_dir.join("journal.db").display()
            )],
        ),
        Err(error) => refuse(&error),
    };
    if view.json {
        println!("{}", ResponseAdapter::seal(&envelope(rows, notes))?.json());
        return Ok(());
    }
    if rows.is_empty() && !notes.is_empty() {
        for note in &notes {
            eprintln!("magistr workspace: {note}");
        }
        return Ok(());
    }
    println!("{}", JournalRow::render_text(&rows));
    Ok(())
}

/// A journal this binary cannot read is a data problem (65), like the
/// server's own fail-closed path; anything else is an ordinary failure.
fn refuse(error: &WorkspaceError) -> ! {
    let code = if error.kind == ErrorKind::UnsupportedVersion {
        WorkspaceOpenError::EXIT_DATA_ERROR
    } else {
        1
    };
    fail(code, &error.message)
}

/// The `journal` CLI's envelope — the shape `undo action:"list"` returns,
/// with the retained accounting (`requested: 0`, `used` estimated over the
/// rows) since the CLI never budgets a listing.
fn envelope(rows: Vec<JournalRow>, notes: Vec<String>) -> WorkspaceResponse<JournalRow> {
    let used = serde_json::to_string(&rows).map_or(0, |text| {
        text.encode_utf16().count().div_ceil(CHARS_PER_TOKEN)
    }) as u64;
    WorkspaceResponse::Success {
        version: EnvelopeVersion::LATEST,
        freshness: Freshness::Warm,
        budget: Budget { requested: 0, used },
        results: ResultSet::complete(rows),
        notes: (!notes.is_empty()).then_some(notes),
    }
}

fn seq(value: u64) -> i64 {
    i64::try_from(value).unwrap_or(i64::MAX)
}
