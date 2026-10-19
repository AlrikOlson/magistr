use super::*;
use crate::harness::FakeWorkspace;
use crate::workspace::{JournalEntry, RunRow};

#[test]
fn open_reads_through_the_port_and_carries_the_content() {
    let mut wb = FakeWorkspace::new().with_file("src/lib.rs", "a\nb\nc\n");
    let event = apply(
        &mut wb,
        &WorkspaceCmd::Open {
            path: "src/lib.rs".to_owned(),
        },
    );
    assert_eq!(
        event,
        WorkspaceEvent::Opened {
            path: "src/lib.rs".to_owned(),
            content: "a\nb\nc\n".to_owned(),
        }
    );
}

#[test]
fn undo_rewinds_through_the_port() {
    let mut wb = FakeWorkspace::new().with_edit("e1");
    let event = apply(
        &mut wb,
        &WorkspaceCmd::Undo {
            id: "e1".to_owned(),
        },
    );
    assert_eq!(
        event,
        WorkspaceEvent::Undone {
            id: "e1".to_owned()
        }
    );
    assert!(wb.journal_entries().is_empty());
}

#[test]
fn journal_lists_the_entries_through_the_port() {
    let entry = JournalEntry {
        at_ms: None,
        id: "e1".to_owned(),
        path: "src/lib.rs".to_owned(),
        added: 2,
        removed: 0,
        parse_ok: true,
        diff: "+a\n+b".to_owned(),
    };
    let mut wb = FakeWorkspace::new().with_change(entry.clone(), "");
    let event = apply(&mut wb, &WorkspaceCmd::Journal);
    assert_eq!(
        event,
        WorkspaceEvent::Journal {
            entries: vec![entry]
        }
    );
}

#[test]
fn search_map_and_run_answer_through_the_port() {
    let rows = vec![RunRow {
        name: "tests::alpha".to_owned(),
        passed: false,
        detail: "boom".to_owned(),
    }];
    let mut wb = FakeWorkspace::new()
        .with_file("a.rs", "fn alpha() {}\n")
        .with_run("test", rows.clone());
    assert_eq!(
        apply(
            &mut wb,
            &WorkspaceCmd::Search {
                query: "alpha".to_owned()
            }
        ),
        WorkspaceEvent::Found {
            query: "alpha".to_owned(),
            hits: vec!["a.rs:1: fn alpha() {}".to_owned()],
        }
    );
    assert_eq!(
        apply(&mut wb, &WorkspaceCmd::Map),
        WorkspaceEvent::Mapped {
            lines: vec!["a.rs".to_owned()],
        }
    );
    assert_eq!(
        apply(
            &mut wb,
            &WorkspaceCmd::Run {
                command: "test".to_owned()
            }
        ),
        WorkspaceEvent::Ran {
            command: "test".to_owned(),
            rows,
        }
    );
}

#[test]
fn a_refusal_becomes_a_failed_event_with_the_reason() {
    let mut wb = FakeWorkspace::new();
    let event = apply(
        &mut wb,
        &WorkspaceCmd::Undo {
            id: "e9".to_owned(),
        },
    );
    assert_eq!(
        event,
        WorkspaceEvent::Failed {
            cmd: WorkspaceCmd::Undo {
                id: "e9".to_owned(),
            },
            error: "no journal entry for e9".to_owned(),
        }
    );
    assert_eq!(
        apply(
            &mut wb,
            &WorkspaceCmd::Run {
                command: "test".to_owned()
            }
        ),
        WorkspaceEvent::Failed {
            cmd: WorkspaceCmd::Run {
                command: "test".to_owned()
            },
            error: "no declared command: test".to_owned(),
        }
    );
}
