use super::*;
use crate::{
    file_content::FileContent,
    journal::{JournalDbOptions, JournalFileChange, JournalFileKind, JournalOpInit},
};

fn seeded() -> (tempfile::TempDir, JournalDb) {
    let dir = tempfile::tempdir().expect("workspace");
    let state = dir.path().join(".magistr/workspace");
    std::fs::create_dir_all(&state).expect("state dir");
    let mut journal =
        JournalDb::open(JournalDbOptions::new(state.join("journal.db"))).expect("journal");
    let ops = [
        ("edit", "alice", "src/a.ts", "", "const a = 1\n"),
        ("files", "bob", "src/a.ts", "const a = 1\n", "const a = 2\n"),
        ("edit", "alice", "src/b.ts", "", "export const b = 3\n"),
    ];
    for (tool, session, path, before, after) in ops {
        journal
            .append(&JournalOpInit {
                tool: tool.into(),
                args_hash: format!("{tool}-{path}"),
                session_id: session.into(),
                files: vec![JournalFileChange {
                    kind: if before.is_empty() {
                        JournalFileKind::Create
                    } else {
                        JournalFileKind::Write
                    },
                    path: path.into(),
                    before: FileContent::Text(before.into()),
                    after: FileContent::Text(after.into()),
                }],
                undo_of: None,
            })
            .expect("append");
    }
    (dir, journal)
}

fn rows(response: &WorkspaceResponse<JournalRow>) -> (Vec<JournalRow>, Option<Vec<String>>) {
    match response {
        WorkspaceResponse::Success { results, notes, .. } => {
            (results.rows().to_vec(), notes.clone())
        }
        WorkspaceResponse::Failure { error, .. } => panic!("expected rows, got {error}"),
    }
}

#[test]
fn lists_newest_first_across_sessions_with_diffs_and_no_cut() {
    let (_dir, journal) = seeded();
    let response = list(&journal, &JournalListInput::default(), "");
    let (listed, notes) = rows(&response);
    assert_eq!(
        listed.iter().map(|row| row.seq).collect::<Vec<_>>(),
        vec![3, 2, 1]
    );
    assert_eq!(listed[0].path.as_deref(), Some("src/b.ts"));
    assert!(
        listed[0]
            .diff
            .as_deref()
            .is_some_and(|d| d.contains("+export const b = 3"))
    );
    assert!(listed.iter().all(|row| row.undoable));
    assert_eq!(notes, None, "workspace scope with rows carries no note");
    let WorkspaceResponse::Success {
        results, budget, ..
    } = &response
    else {
        unreachable!()
    };
    assert!(results.truncation().is_none());
    let json = serde_json::to_string(results.rows()).expect("json");
    assert_eq!(
        budget.used as usize,
        json.encode_utf16().count().div_ceil(4)
    );
}

#[test]
fn a_limit_probes_one_op_further_and_hands_back_a_real_cursor() {
    let (_dir, journal) = seeded();
    let input = JournalListInput {
        limit: 1,
        ..JournalListInput::default()
    };
    let response = list(&journal, &input, "");
    let WorkspaceResponse::Success { results, notes, .. } = &response else {
        panic!("rows expected");
    };
    assert_eq!(results.rows().len(), 1);
    assert_eq!(results.rows()[0].seq, 3);
    let cut = results
        .truncation()
        .expect("the probe op proves there is more");
    assert_eq!(cut.dropped(), 1);
    assert_eq!(cut.reason().as_str(), "ceiling");
    assert_eq!(cut.cursor(), "beforeSeq:3");
    assert_eq!(
        notes.as_deref(),
        Some(&["undo: older ops remain — continue with beforeSeq: 3".to_owned()][..])
    );
    let next = list(
        &journal,
        &JournalListInput {
            before_seq: Some(3),
            ..JournalListInput::default()
        },
        "",
    );
    assert_eq!(
        rows(&next).0.iter().map(|row| row.seq).collect::<Vec<_>>(),
        vec![2, 1]
    );
}

#[test]
fn session_scope_filters_and_an_anonymous_session_is_noted() {
    let (_dir, journal) = seeded();
    let input = JournalListInput {
        scope: UndoScope::Session,
        ..JournalListInput::default()
    };
    let (alice, notes) = rows(&list(&journal, &input, "alice"));
    assert_eq!(
        alice.iter().map(|row| row.seq).collect::<Vec<_>>(),
        vec![3, 1]
    );
    assert_eq!(notes, None);
    let (anonymous, notes) = rows(&list(&journal, &input, ""));
    assert!(anonymous.is_empty());
    let notes = notes.expect("notes");
    assert!(notes[0].contains("no session identity was supplied"));
    assert_eq!(notes[1], "undo: the journal has no ops in session scope");
}

#[test]
fn diff_false_keeps_the_counts_and_drops_the_bodies() {
    let (_dir, journal) = seeded();
    let input = JournalListInput {
        diff: false,
        ..JournalListInput::default()
    };
    let (listed, _) = rows(&list(&journal, &input, ""));
    assert!(listed.iter().all(|row| row.diff.is_none()));
    assert_eq!((listed[0].added, listed[0].removed), (1, 0));
}

#[test]
fn a_tiny_budget_cuts_on_an_op_boundary_but_never_to_zero() {
    let (_dir, journal) = seeded();
    let input = JournalListInput {
        budget: 1,
        ..JournalListInput::default()
    };
    let response = list(&journal, &input, "");
    let WorkspaceResponse::Success { results, notes, .. } = &response else {
        panic!("rows expected");
    };
    assert_eq!(
        results.rows().len(),
        1,
        "one over-budget op beats an empty log"
    );
    let cut = results.truncation().expect("cut");
    assert_eq!(cut.reason().as_str(), "budget");
    assert_eq!(cut.dropped(), 2);
    assert!(notes.as_ref().expect("note")[0].contains("cut to fit the 1-token budget"));
}

#[test]
fn since_seq_tails_the_log() {
    let (_dir, journal) = seeded();
    let input = JournalListInput {
        since_seq: Some(2),
        ..JournalListInput::default()
    };
    assert_eq!(
        rows(&list(&journal, &input, ""))
            .0
            .iter()
            .map(|row| row.seq)
            .collect::<Vec<_>>(),
        vec![3, 2]
    );
}

#[test]
fn schema_level_misuse_is_a_typed_narrow_refusal() {
    let (_dir, journal) = seeded();
    for (input, expected) in [
        (
            JournalListInput {
                limit: 0,
                ..JournalListInput::default()
            },
            "undo: limit must be an integer in [1, 200]",
        ),
        (
            JournalListInput {
                since_seq: Some(-1),
                ..JournalListInput::default()
            },
            "undo: sinceSeq must be a non-negative integer journal seq",
        ),
    ] {
        let WorkspaceResponse::Failure { error, .. } = list(&journal, &input, "") else {
            panic!("expected a refusal");
        };
        assert_eq!(error.kind, ErrorKind::PatternInvalid);
        assert_eq!(error.message, expected);
    }
}

#[test]
fn an_empty_workspace_lists_nothing_with_the_retained_accounting() {
    let dir = tempfile::tempdir().expect("workspace");
    let state = dir.path().join(".magistr/workspace");
    std::fs::create_dir_all(&state).expect("state dir");
    let journal =
        JournalDb::open(JournalDbOptions::new(state.join("journal.db"))).expect("journal");
    let WorkspaceResponse::Success { budget, notes, .. } =
        list(&journal, &JournalListInput::default(), "")
    else {
        panic!("rows expected");
    };
    assert_eq!((budget.requested, budget.used), (8000, 1));
    assert_eq!(
        notes,
        Some(vec![
            "undo: the journal has no ops in workspace scope".to_owned()
        ])
    );
}
