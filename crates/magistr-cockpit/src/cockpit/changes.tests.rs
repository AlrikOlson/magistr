use super::*;

fn entry(id: &str) -> JournalEntry {
    JournalEntry {
        at_ms: None,
        id: id.to_owned(),
        path: format!("src/{id}.rs"),
        added: 1,
        removed: 0,
        parse_ok: true,
        diff: "+line".to_owned(),
    }
}

fn row(name: &str, passed: bool) -> RunRow {
    RunRow {
        name: name.to_owned(),
        passed,
        detail: String::new(),
    }
}

#[test]
fn a_journal_refresh_replaces_the_list_and_keeps_surviving_flags() {
    let mut changes = Changes::default();
    changes.set_entries(vec![entry("e1"), entry("e2")]);
    changes.toggle(0);
    changes.approve(1);
    assert!(changes.is_expanded("e1"));
    assert!(changes.is_approved("e2"));

    changes.set_entries(vec![entry("e2"), entry("e3")]);
    assert!(!changes.is_expanded("e1"), "the expansion left with e1");
    assert!(changes.is_approved("e2"), "the approval survives");
    assert_eq!(changes.entries().len(), 2);
}

#[test]
fn one_entry_expands_at_a_time_and_a_second_click_folds_it() {
    let mut changes = Changes::default();
    changes.set_entries(vec![entry("e1"), entry("e2")]);
    changes.toggle(0);
    changes.toggle(1);
    assert!(!changes.is_expanded("e1"));
    assert!(changes.is_expanded("e2"));
    changes.toggle(1);
    assert!(!changes.is_expanded("e2"));
    changes.toggle(9);
    assert!(!changes.is_expanded("e1") && !changes.is_expanded("e2"));
}

#[test]
fn removing_an_entry_drops_it_and_its_flags() {
    let mut changes = Changes::default();
    changes.set_entries(vec![entry("e1"), entry("e2")]);
    changes.toggle(0);
    changes.approve(0);
    changes.remove("e1");
    assert_eq!(changes.entries().len(), 1);
    assert_eq!(changes.entries()[0].id, "e2");
    assert!(!changes.is_expanded("e1"));
    assert!(!changes.is_approved("e1"));
}

#[test]
fn run_rows_sort_failing_first_keeping_their_order() {
    let mut changes = Changes::default();
    changes.set_runs(
        "test",
        vec![
            row("a::passes", true),
            row("b::fails", false),
            row("c::fails", false),
            row("d::passes", true),
        ],
    );
    let (command, rows) = changes.runs().expect("a run landed");
    assert_eq!(command, "test");
    let names: Vec<&str> = rows.iter().map(|row| row.name.as_str()).collect();
    assert_eq!(names, ["b::fails", "c::fails", "a::passes", "d::passes"]);
}

#[test]
fn the_viewer_opens_and_closes() {
    let mut changes = Changes::default();
    assert!(changes.is_empty());
    assert!(!changes.close_viewer());
    changes.open_viewer("src/lib.rs", vec!["a".to_owned(), "b".to_owned()]);
    let (title, lines) = changes.viewer().expect("the pane is open");
    assert_eq!(title, "src/lib.rs");
    assert_eq!(lines, ["a", "b"]);
    assert!(!changes.is_empty());
    assert!(changes.close_viewer());
    assert!(changes.viewer().is_none());
}

#[test]
fn the_body_counts_the_rows_the_tab_paints() {
    let mut changes = Changes::default();
    // Nothing to scroll — the pinned heading is the empty state.
    assert_eq!(changes.body_rows(), 0);
    changes.set_entries(vec![
        JournalEntry {
            id: "e1".to_owned(),
            path: "src/a.rs".to_owned(),
            diff: "@@ -1 +1 @@\n-old\n+new".to_owned(),
            ..JournalEntry::default()
        },
        JournalEntry {
            id: "e2".to_owned(),
            path: "src/b.rs".to_owned(),
            ..JournalEntry::default()
        },
    ]);
    // A head row each, nothing expanded.
    assert_eq!(changes.body_rows(), 2);
    changes.toggle(0);
    // Inspector disclosure leaves the compact list unchanged.
    assert_eq!(changes.body_rows(), 2);
    changes.set_runs("test", vec![row("a::passes", true), row("b::fails", false)]);
    // A row per result. The run's own heading is the pinned verdict
    // strip now, so it is no longer one of the body's rows.
    assert_eq!(changes.body_rows(), 4);
}

#[test]
fn the_viewer_pane_bodies_only_its_lines_because_its_title_is_pinned() {
    let mut changes = Changes::default();
    changes.open_viewer("src/lib.rs", vec!["a".to_owned(), "b".to_owned()]);
    assert_eq!(changes.pinned_rows(), 1, "the pane's own heading");
    assert_eq!(changes.body_rows(), 2, "and its two lines scroll under it");
}

// ── The pinned chrome ──────────────────────

/// What stays on screen while the journal moves under it: the tab's
/// heading always, and the run verdict once a run has landed. Bounded
/// at two rows whatever the journal or the run does — the result rows
/// themselves stay in the body, where they can be windowed.
#[test]
fn the_heading_is_always_pinned_and_the_run_verdict_joins_it() {
    let mut changes = Changes::default();
    assert_eq!(changes.pinned_rows(), 1, "the heading, on an empty tab");
    assert!(changes.run_summary().is_none(), "no run has landed yet");

    changes.set_entries(vec![entry("e1"), entry("e2")]);
    assert_eq!(changes.pinned_rows(), 1, "a journal pins no more");

    changes.set_runs("test", vec![row("a::passes", true)]);
    assert_eq!(changes.pinned_rows(), 2, "the heading and the verdict");

    // A run answering a hundred rows still pins exactly two: the strip
    // is a verdict, not the result list.
    let many: Vec<RunRow> = (0..100).map(|n| row(&format!("t{n}"), true)).collect();
    changes.set_runs("test", many);
    assert_eq!(changes.pinned_rows(), 2, "bounded whatever the run said");

    // The viewer pane covers the journal the run belongs to and wears
    // its own title, so the strip stands down while it is open.
    changes.open_viewer("src/lib.rs", vec!["a".to_owned()]);
    assert!(changes.run_summary().is_none(), "the pane covers it");
    assert_eq!(changes.pinned_rows(), 1, "the pane's title alone");
    assert!(changes.close_viewer());
    assert_eq!(changes.pinned_rows(), 2, "and the verdict comes back");
}

/// The verdict reads the failures out, because that is what brings a
/// user back to the tab; an all-green run says how many passed.
#[test]
fn the_run_verdict_counts_the_failures_first() {
    let mut changes = Changes::default();
    changes.set_runs("test", vec![row("a", true), row("b", true)]);
    assert_eq!(
        changes.run_summary().as_deref(),
        Some("run test · 2 passed")
    );

    changes.set_runs(
        "test",
        vec![row("a", true), row("b", false), row("c", false)],
    );
    assert_eq!(
        changes.run_summary().as_deref(),
        Some("run test · 2 failed of 3")
    );

    // A command that answered nothing still says it ran.
    changes.set_runs("lint", Vec::new());
    assert_eq!(
        changes.run_summary().as_deref(),
        Some("run lint · no results")
    );
}

/// The heading names the tab and what the journal holds, so a scrolled
/// list still says how much of it there is.
#[test]
fn the_heading_counts_the_journal_and_its_lines() {
    let mut changes = Changes::default();
    assert_eq!(changes.heading(), "changes · nothing journaled yet");
    changes.set_entries(vec![
        JournalEntry {
            id: "e1".to_owned(),
            path: "src/a.rs".to_owned(),
            added: 3,
            removed: 1,
            ..JournalEntry::default()
        },
        JournalEntry {
            id: "e2".to_owned(),
            path: "src/b.rs".to_owned(),
            added: 4,
            removed: 0,
            ..JournalEntry::default()
        },
    ]);
    assert_eq!(changes.heading(), "changes · 2 files · +7 −1");
    changes.open_viewer("src/lib.rs", vec!["a".to_owned()]);
    assert_eq!(
        changes.heading(),
        "src/lib.rs",
        "the viewer pane names the file it is showing instead"
    );
}
