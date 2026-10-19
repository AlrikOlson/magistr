use super::*;

fn entry(id: &str, path: &str) -> JournalEntry {
    JournalEntry {
        at_ms: None,
        id: id.to_owned(),
        path: path.to_owned(),
        added: 1,
        removed: 1,
        parse_ok: true,
        diff: "@@ -1 +1 @@\n-old\n+new".to_owned(),
    }
}

#[test]
fn reads_answer_scripted_files_and_refuse_unknown_paths() {
    let mut wb = FakeWorkspace::new().with_file("src/main.rs", "fn main() {}\n");
    assert_eq!(wb.read("src/main.rs"), Ok("fn main() {}\n".to_owned()));
    assert!(wb.read("nope.rs").is_err());
    assert_eq!(wb.calls(), ["read src/main.rs", "read nope.rs"]);
}

#[test]
fn undo_rewinds_the_journal_once_and_refuses_twice() {
    let mut wb = FakeWorkspace::new().with_edit("e1");
    assert_eq!(wb.undo("e1"), Ok(()));
    assert!(wb.journal_entries().is_empty(), "the entry is rewound away");
    assert!(wb.undo("e1").is_err(), "a second rewind is refused");
    assert!(wb.undo("e2").is_err());
    assert_eq!(wb.calls(), ["undo e1", "undo e1", "undo e2"]);
}

#[test]
fn undoing_a_change_restores_the_pre_edit_content() {
    let mut wb = FakeWorkspace::new()
        .with_file("src/lib.rs", "new\n")
        .with_change(entry("e1", "src/lib.rs"), "old\n");
    assert_eq!(wb.undo("e1"), Ok(()));
    assert_eq!(wb.file("src/lib.rs"), Some("old\n"));
    assert!(wb.journal_entries().is_empty());
}

#[test]
fn the_journal_lists_its_entries_oldest_first() {
    let mut wb = FakeWorkspace::new()
        .with_change(entry("e1", "a.rs"), "")
        .with_change(entry("e2", "b.rs"), "");
    let entries = wb.journal().expect("the journal reads");
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].id, "e1");
    assert_eq!(entries[1].id, "e2");
}

#[test]
fn search_finds_matching_lines_across_the_files() {
    let mut wb = FakeWorkspace::new()
        .with_file("b.rs", "fn beta() {}\n")
        .with_file("a.rs", "fn alpha() {}\nlet x = 1;\n");
    assert_eq!(
        wb.search("fn "),
        Ok(vec![
            "a.rs:1: fn alpha() {}".to_owned(),
            "b.rs:1: fn beta() {}".to_owned(),
        ])
    );
    assert_eq!(wb.search("nothing"), Ok(Vec::new()));
}

#[test]
fn map_lists_the_files_sorted() {
    let mut wb = FakeWorkspace::new()
        .with_file("src/b.rs", "")
        .with_file("src/a.rs", "");
    assert_eq!(
        wb.map(),
        Ok(vec!["src/a.rs".to_owned(), "src/b.rs".to_owned()])
    );
}

#[test]
fn run_answers_scripted_rows_and_refuses_undeclared_commands() {
    let rows = vec![RunRow {
        name: "tests::alpha".to_owned(),
        passed: false,
        detail: "assertion failed".to_owned(),
    }];
    let mut wb = FakeWorkspace::new().with_run("test", rows.clone());
    assert_eq!(wb.run("test"), Ok(rows));
    assert!(wb.run("build").is_err());
    assert_eq!(wb.calls(), ["run test", "run build"]);
}
