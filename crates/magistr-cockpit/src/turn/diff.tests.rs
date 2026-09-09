use super::*;

#[test]
fn a_changed_line_is_a_remove_and_an_add_with_context() {
    let old = "fn main() {\n    println!(\"hi\");\n}\n";
    let new = "fn main() {\n    println!(\"hello\");\n}\n";
    let body = body(old, new);
    let lines: Vec<&str> = body.lines().collect();
    assert!(lines[0].starts_with("@@"), "a hunk header leads: {body}");
    assert!(lines.contains(&"-    println!(\"hi\");"), "{body}");
    assert!(lines.contains(&"+    println!(\"hello\");"), "{body}");
    assert!(
        lines.contains(&" fn main() {"),
        "context rides along: {body}"
    );
}

#[test]
fn distant_changes_split_into_hunks() {
    let old: String = (0..30).map(|i| format!("line {i}\n")).collect();
    let new = old
        .replace("line 2\n", "line two\n")
        .replace("line 27\n", "line twenty-seven\n");
    let body = body(&old, &new);
    let hunks = body.lines().filter(|l| l.starts_with("@@")).count();
    assert_eq!(hunks, 2, "{body}");
}

#[test]
fn a_new_file_is_every_line_added() {
    assert_eq!(body("", "a\nb\n"), "+a\n+b");
}

#[test]
fn an_emptied_file_is_every_line_removed() {
    let body = body("a\nb\n", "");
    assert!(body.lines().any(|l| l == "-a"), "{body}");
    assert!(body.lines().any(|l| l == "-b"), "{body}");
}

#[test]
fn no_change_and_no_text_are_empty() {
    assert_eq!(body("", ""), "");
    assert_eq!(body("same\n", "same\n"), "");
}

#[test]
fn the_body_has_no_trailing_newline() {
    let body = body("a\n", "b\n");
    assert!(!body.ends_with('\n'), "{body:?}");
}
