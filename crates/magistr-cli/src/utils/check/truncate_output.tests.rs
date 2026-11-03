use super::*;

#[test]
fn truncate_output_below_limit_unchanged() {
    let small = "hello world";
    assert_eq!(truncate_output(small, 100), small);
}

#[test]
fn truncate_output_at_limit_unchanged() {
    let exact = "a".repeat(100);
    assert_eq!(truncate_output(&exact, 100), exact);
}

#[test]
fn truncate_output_above_limit_has_marker() {
    let big = "x".repeat(200);
    let result = truncate_output(&big, 100);
    assert!(result.contains("[... truncated, 200 bytes total ...]"));
    assert!(result.len() < 200);
}

#[test]
fn truncate_output_preserves_head_and_tail() {
    // Build output with identifiable head and tail
    let head = "HEAD_".repeat(20); // 100 chars
    let tail = "_TAIL".repeat(20); // 100 chars
    let middle = "M".repeat(200);
    let big = format!("{head}{middle}{tail}");
    let result = truncate_output(&big, 200);
    assert!(result.starts_with("HEAD_"), "should preserve head");
    assert!(result.ends_with("_TAIL"), "should preserve tail");
    assert!(result.contains("[... truncated,"));
}
