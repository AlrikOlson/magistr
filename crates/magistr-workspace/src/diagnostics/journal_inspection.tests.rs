use std::collections::BTreeMap;

use super::*;
use crate::{
    file_content::FileContent,
    journal::{JournalDb, JournalDbOptions, JournalFileChange, JournalFileKind, JournalOpInit},
};

fn seeded() -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("workspace");
    let state = dir.path().join(".workbench");
    std::fs::create_dir_all(&state).expect("state dir");
    let mut journal =
        JournalDb::open(JournalDbOptions::new(state.join("journal.db"))).expect("journal");
    journal
        .append(&JournalOpInit {
            tool: "edit".into(),
            args_hash: "h1".into(),
            session_id: "alice".into(),
            files: vec![JournalFileChange {
                kind: JournalFileKind::Create,
                path: "src/a.ts".into(),
                before: FileContent::Text(String::new()),
                after: FileContent::Text("const a = 1\n".into()),
            }],
            undo_of: None,
        })
        .expect("first op");
    journal
        .append(&JournalOpInit {
            tool: "files".into(),
            args_hash: "h2".into(),
            session_id: "bob".into(),
            files: vec![JournalFileChange {
                kind: JournalFileKind::Write,
                path: "src/a.ts".into(),
                before: FileContent::Text("const a = 1\n".into()),
                after: FileContent::Text("const a = 2\n".into()),
            }],
            undo_of: None,
        })
        .expect("second op");
    dir
}

fn rows(response: WorkspaceResponse<JournalRow>) -> Vec<JournalRow> {
    match response {
        WorkspaceResponse::Success { results, .. } => results.rows().to_vec(),
        WorkspaceResponse::Failure { error, .. } => panic!("expected rows, got {error}"),
    }
}

/// Hashes of everything under the state dir except the WAL shared-memory
/// index: SQLite mutates that sidecar for any reader.
fn snapshot(state: &std::path::Path) -> BTreeMap<String, Vec<u8>> {
    std::fs::read_dir(state)
        .expect("state dir")
        .filter_map(Result::ok)
        .filter(|entry| entry.path().is_file())
        .filter(|entry| !entry.file_name().to_string_lossy().ends_with("-shm"))
        .map(|entry| {
            (
                entry.file_name().to_string_lossy().into_owned(),
                std::fs::read(entry.path()).expect("state file"),
            )
        })
        .collect()
}

#[test]
fn lists_the_log_as_rows_newest_op_first_with_diffs() {
    let dir = seeded();
    let state = dir.path().join(".workbench");
    let rows = rows(JournalInspection::list(
        &state,
        &JournalLogQuery::default(),
        &JournalViewOptions::default(),
        4000,
    ));
    assert_eq!(
        rows.iter().map(|row| row.seq).collect::<Vec<_>>(),
        vec![2, 1]
    );
    let newest = &rows[0];
    assert_eq!(newest.tool, "files");
    assert_eq!(newest.session_id, "bob");
    assert_eq!(newest.path.as_deref(), Some("src/a.ts"));
    assert_eq!((newest.added, newest.removed), (1, 1));
    assert!(newest.undoable);
    assert_eq!(newest.change, Some("write"));
    assert_eq!(newest.parse_ok, Some(true));
    assert!(
        newest
            .diff
            .as_deref()
            .expect("diff")
            .contains("+const a = 2")
    );
    assert_eq!(rows[1].change, Some("create"));
    assert_eq!((rows[1].added, rows[1].removed), (1, 0));
    let wire = serde_json::to_value(newest).expect("json");
    assert_eq!(wire["sessionId"], "bob");
    assert!(wire.get("undoneBy").is_none());
}

#[test]
fn runs_beside_a_live_writer_without_touching_the_state_dir() {
    let dir = seeded();
    let state = dir.path().join(".workbench");
    std::fs::write(state.join("server.lock"), "{\"pid\":1}").expect("lock");
    // Keep a writer open as a real owner does.
    let writer = JournalDb::open(JournalDbOptions::new(state.join("journal.db"))).expect("writer");
    let before = snapshot(&state);
    let listed = rows(JournalInspection::list(
        &state,
        &JournalLogQuery::default(),
        &JournalViewOptions::default(),
        4000,
    ));
    assert_eq!(listed.len(), 2);
    assert!(state.join("server.lock").exists());
    assert_eq!(snapshot(&state), before);
    drop(writer);
}

#[test]
fn limit_before_since_session_and_no_diff_narrow_the_listing() {
    let dir = seeded();
    let state = dir.path().join(".workbench");
    let list = |query: JournalLogQuery, options: JournalViewOptions| {
        rows(JournalInspection::list(&state, &query, &options, 4000))
    };
    let seqs = |rows: Vec<JournalRow>| rows.iter().map(|row| row.seq).collect::<Vec<_>>();
    assert_eq!(
        seqs(list(
            JournalLogQuery {
                limit: 1,
                ..Default::default()
            },
            JournalViewOptions::default()
        )),
        vec![2]
    );
    assert_eq!(
        seqs(list(
            JournalLogQuery {
                before_seq: Some(2),
                ..Default::default()
            },
            JournalViewOptions::default()
        )),
        vec![1]
    );
    assert_eq!(
        seqs(list(
            JournalLogQuery {
                since_seq: Some(2),
                ..Default::default()
            },
            JournalViewOptions::default()
        )),
        vec![2]
    );
    assert_eq!(
        seqs(list(
            JournalLogQuery {
                session_id: Some("alice".into()),
                ..Default::default()
            },
            JournalViewOptions::default()
        )),
        vec![1]
    );
    let no_diff = list(
        JournalLogQuery::default(),
        JournalViewOptions {
            diff: false,
            ..Default::default()
        },
    );
    assert!(no_diff[0].diff.is_none());
    assert_eq!(no_diff[0].added, 1);
    let cut = list(
        JournalLogQuery::default(),
        JournalViewOptions {
            diff: true,
            diff_char_cap: 20,
        },
    );
    assert_eq!(cut[0].diff_truncated, Some(true));
    assert!(cut[0].diff.as_ref().expect("diff").encode_utf16().count() <= 20);
}

#[test]
fn a_workspace_that_never_wrote_says_so_and_a_newer_journal_fails_closed() {
    let empty = tempfile::tempdir().expect("workspace");
    let state = empty.path().join(".workbench");
    let fresh = JournalInspection::list(
        &state,
        &JournalLogQuery::default(),
        &JournalViewOptions::default(),
        4000,
    );
    let WorkspaceResponse::Success { results, notes, .. } = fresh else {
        panic!("a missing journal is an empty listing");
    };
    assert!(results.rows().is_empty());
    assert!(
        notes
            .expect("note")
            .iter()
            .any(|note| note.contains("no journal"))
    );
    assert!(!state.exists(), "inspection never creates state");

    std::fs::create_dir_all(&state).expect("state dir");
    let path = state.join("journal.db");
    let raw = rusqlite::Connection::open(&path).expect("raw journal");
    raw.execute_batch("PRAGMA user_version = 999")
        .expect("future version");
    drop(raw);
    let before = std::fs::read(&path).expect("bytes");
    let refused = JournalInspection::list(
        &state,
        &JournalLogQuery::default(),
        &JournalViewOptions::default(),
        4000,
    );
    let WorkspaceResponse::Failure { error, .. } = refused else {
        panic!("a newer journal fails closed");
    };
    assert_eq!(error.kind, ErrorKind::UnsupportedVersion);
    assert!(error.message.contains("cannot be read"));
    assert_eq!(std::fs::read(&path).expect("bytes"), before);
}

#[test]
fn a_fileless_op_and_an_evicted_blob_still_get_rows() {
    use crate::journal::{JournalFileRecord, JournalLogEntry, JournalOp};
    let op = |seq: i64, files: Vec<JournalFileRecord>| JournalLogEntry {
        op: JournalOp {
            seq,
            at: 1,
            tool: "undo".into(),
            args_hash: "h".into(),
            session_id: String::new(),
            undo_of: Some(vec![1]),
            files,
        },
        undoable: false,
        undone_by: None,
        write_outcome: None,
    };
    let evicted = JournalFileRecord {
        kind: JournalFileKind::Write,
        path: "big.bin".into(),
        before_hash: "a".repeat(64),
        after_hash: "b".repeat(64),
        before: None,
        after: Some(FileContent::Binary(vec![0, 1])),
    };
    let rows = JournalRow::render(
        &[op(3, vec![]), op(4, vec![evicted])],
        &JournalViewOptions::default(),
    );
    assert_eq!(rows[0].parse_ok, Some(true));
    assert_eq!(rows[0].undo_of, Some(vec![1]));
    assert!(rows[0].path.is_none());
    assert_eq!(rows[1].evicted, Some(true));
    assert_eq!(rows[1].path.as_deref(), Some("big.bin"));
    assert!(rows[1].diff.is_none());
}
