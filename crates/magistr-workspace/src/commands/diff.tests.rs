use super::*;

fn text(value: &str) -> FileContent {
    FileContent::Text(value.into())
}

#[test]
fn identical_states_render_nothing_and_count_zero() {
    let same = text("a\n");
    assert_eq!(unified_diff("x.txt", Some(&same), Some(&same)), "");
    assert_eq!(unified_diff("x.txt", None, None), "");
    assert_eq!(changed_lines(Some(&same), Some(&same)), 0);
}

#[test]
fn creation_deletion_and_edits_use_the_retained_headers_and_count_changed_lines() {
    let created = unified_diff("src/a.ts", None, Some(&text("const a = 1;\n")));
    assert!(
        created.starts_with("--- /dev/null\n+++ b/src/a.ts\n"),
        "{created}"
    );
    assert!(created.contains("+const a = 1;\n"));
    let deleted = unified_diff("src/a.ts", Some(&text("gone\n")), None);
    assert!(
        deleted.starts_with("--- a/src/a.ts\n+++ /dev/null\n"),
        "{deleted}"
    );
    let before = text("one\ntwo\nthree\n");
    let after = text("one\n2\nthree\nfour\n");
    let edit = unified_diff("n.txt", Some(&before), Some(&after));
    assert!(edit.contains("-two\n+2\n"), "{edit}");
    assert_eq!(changed_lines(Some(&before), Some(&after)), 3);
    assert_eq!(changed_lines(None, Some(&after)), 4);
}

#[test]
fn a_missing_trailing_newline_is_reported_not_hidden() {
    let diff = unified_diff("t.txt", Some(&text("line\n")), Some(&text("line")));
    assert!(diff.contains("No newline at end of file"), "{diff}");
    assert_eq!(changed_lines(Some(&text("line\n")), Some(&text("line"))), 2);
}

#[test]
fn binary_transitions_are_summarized_by_bytes_and_hash() {
    let binary = FileContent::Binary(vec![0, 1, 2]);
    let diff = unified_diff("blob.bin", Some(&text("x\n")), Some(&binary));
    assert!(diff.starts_with("Binary files a/blob.bin and b/blob.bin differ\n"));
    assert!(diff.contains("After: 3 bytes, sha256 "));
    assert_eq!(changed_lines(Some(&text("x\n")), Some(&binary)), 1);
}
