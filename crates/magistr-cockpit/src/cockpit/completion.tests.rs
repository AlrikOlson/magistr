use super::*;

fn files() -> Vec<String> {
    ["src/main.rs", "src/cockpit/prompt.rs", "README.md"]
        .map(str::to_owned)
        .to_vec()
}

#[test]
fn file_completion_filters_case_insensitively_as_the_query_grows() {
    let mut c = Completion::file(0);
    assert!(c.refresh("@", 1, &files()), "an empty query lists files");
    assert_eq!(c.items().len(), 3);
    assert!(c.refresh("@PROMPT", 7, &files()));
    assert_eq!(c.items(), ["src/cockpit/prompt.rs"]);
    assert_eq!(
        c.replacement().as_deref(),
        Some("@src/cockpit/prompt.rs "),
        "a file keeps its trigger"
    );
    assert!(!c.refresh("@zzz", 4, &files()), "no match closes");
    assert!(!c.refresh("@a b", 4, &files()), "whitespace closes");
    assert!(!c.refresh("a", 1, &files()), "a lost trigger closes");
    assert!(
        !c.refresh("@ab", 0, &files()),
        "a cursor before the query closes"
    );
}

#[test]
fn command_completion_matches_the_prefix_and_keeps_its_slash() {
    let mut c = Completion::command(0);
    assert!(c.refresh("/", 1, &[]));
    assert_eq!(
        c.items().len(),
        MAX_ITEMS,
        "an empty query fills the popup from the command set"
    );
    assert!(c.refresh("/ne", 3, &[]));
    assert_eq!(c.items(), ["/next", "/next-loop"]);
    assert_eq!(c.replacement().as_deref(), Some("/next "));
    assert!(c.refresh("/se", 3, &[]), "the workbench palette completes");
    assert_eq!(c.items(), ["/search"]);
    assert!(!c.refresh("/zzz", 4, &[]), "no such command");
}

#[test]
fn the_selection_moves_with_wrap_clamps_on_refresh_and_takes_a_click() {
    let mut c = Completion::command(0);
    assert!(c.refresh("/", 1, &[]));
    assert_eq!(c.selected(), 0);
    c.up();
    assert_eq!(
        c.selected(),
        MAX_ITEMS - 1,
        "up from the top wraps to the bottom"
    );
    c.down();
    assert_eq!(c.selected(), 0);
    c.down();
    assert_eq!(c.selected(), 1);
    c.select(4);
    assert_eq!(c.selected(), 4);
    c.select(9);
    assert_eq!(c.selected(), 4, "a click past the list is ignored");
    assert_eq!(c.replacement().as_deref(), Some("/map "));
    assert!(c.refresh("/ne", 3, &[]));
    assert_eq!(c.selected(), 1, "the selection clamps to the shorter list");
    let empty = Completion::file(0);
    assert_eq!(empty.replacement(), None, "nothing to accept yet");
}
