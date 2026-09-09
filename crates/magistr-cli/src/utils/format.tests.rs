use super::*;

// Note: Tests that check escape codes rely on the default (color enabled).
// We can't easily set NO_COLOR in tests because OnceLock caches the first call.
// Instead, we test the formatting logic directly.

#[test]
fn success_contains_checkmark() {
    let msg = success("done");
    assert!(msg.contains('✓') || msg.contains("OK"), "got: {msg}");
    assert!(msg.contains("done"));
}

#[test]
fn error_contains_cross() {
    let msg = error("failed");
    assert!(msg.contains('✗') || msg.contains("ERROR"), "got: {msg}");
    assert!(msg.contains("failed"));
}

#[test]
fn warning_contains_icon() {
    let msg = warning("caution");
    assert!(msg.contains('⚠') || msg.contains("WARN"), "got: {msg}");
    assert!(msg.contains("caution"));
}

#[test]
fn info_contains_arrow() {
    let msg = info("starting");
    assert!(msg.contains('→') || msg.contains("INFO"), "got: {msg}");
    assert!(msg.contains("starting"));
}

#[test]
fn dim_wraps_text() {
    let msg = dim("faded");
    assert!(msg.contains("faded"));
}

#[test]
fn bold_wraps_text() {
    let msg = bold("strong");
    assert!(msg.contains("strong"));
}

#[test]
fn icon_pass_is_nonempty() {
    assert!(!icon_pass().is_empty());
}

#[test]
fn icon_fail_is_nonempty() {
    assert!(!icon_fail().is_empty());
}

#[test]
fn icon_warn_is_nonempty() {
    assert!(!icon_warn().is_empty());
}

#[test]
fn header_includes_title() {
    let h = header("Test Section");
    assert!(h.contains("Test Section"));
}

#[test]
fn separator_is_nonempty() {
    assert!(!separator().is_empty());
}

#[test]
fn kv_formats_pair() {
    let result = kv("name", &"magistr");
    assert!(result.contains("name"));
    assert!(result.contains("magistr"));
}

#[test]
fn kv_works_with_numbers() {
    let result = kv("count", &42);
    assert!(result.contains("count"));
    assert!(result.contains("42"));
}

#[test]
fn green_wraps_text() {
    let msg = green("ok");
    assert!(msg.contains("ok"));
}

#[test]
fn red_wraps_text() {
    let msg = red("fail");
    assert!(msg.contains("fail"));
}

#[test]
fn yellow_wraps_text() {
    let msg = yellow("warn");
    assert!(msg.contains("warn"));
}

#[test]
fn cyan_wraps_text() {
    let msg = cyan("info");
    assert!(msg.contains("info"));
}
