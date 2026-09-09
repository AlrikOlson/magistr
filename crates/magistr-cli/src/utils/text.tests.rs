use super::*;

// ── truncate ──────────────────────────────────────────────────

#[test]
fn truncate_short_unchanged() {
    assert_eq!(truncate("hello", 10), "hello");
}

#[test]
fn truncate_exact_length_unchanged() {
    assert_eq!(truncate("hello", 5), "hello");
}

#[test]
fn truncate_long_adds_ellipsis() {
    let result = truncate("hello world", 6);
    assert!(result.ends_with('…'), "got: {result}");
    // 5 visible chars + ellipsis
    assert_eq!(result, "hello…");
}

#[test]
fn truncate_zero_max() {
    assert_eq!(truncate("hello", 0), "");
}

#[test]
fn truncate_one_max() {
    assert_eq!(truncate("hello", 1), "…");
}

#[test]
fn truncate_empty_string() {
    assert_eq!(truncate("", 10), "");
}

#[test]
fn truncate_multiline_uses_first_line() {
    assert_eq!(truncate("first\nsecond\nthird", 50), "first");
}

#[test]
fn truncate_unicode_safe() {
    // "café" has a multi-byte char — truncation must not split it
    let result = truncate("café latte", 5);
    assert!(result.is_char_boundary(result.len()), "not a valid string");
    assert_eq!(result, "café…");
}
