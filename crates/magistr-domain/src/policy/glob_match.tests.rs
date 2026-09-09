use super::*;

#[test]
fn a_pattern_with_no_wildcard_is_equality() {
    assert!(glob_match("Bash", "Bash"));
    assert!(!glob_match("Bash", "Bashful"));
    assert!(!glob_match("Bash", "Read"));
}

#[test]
fn matching_ignores_case() {
    assert!(glob_match("bash", "Bash"));
    assert!(glob_match("MCP__*", "mcp__magistr__gate"));
}

#[test]
fn star_spans_any_run_including_none() {
    assert!(glob_match("*", ""));
    assert!(glob_match("*", "anything at all"));
    assert!(glob_match("mcp__*", "mcp__magistr__gate"));
    assert!(glob_match("*.rs", "src/main.rs"));
    assert!(glob_match("src/*/mod.rs", "src/team/mod.rs"));
    assert!(!glob_match("*.rs", "Cargo.toml"));
}

#[test]
fn question_mark_spans_exactly_one_character() {
    assert!(glob_match("v?", "v2"));
    assert!(!glob_match("v?", "v"));
    assert!(!glob_match("v?", "v20"));
}

#[test]
fn matching_is_anchored_at_both_ends() {
    // A bare filename does NOT match a path that merely ends with it —
    // the caller must say so with a leading star.
    assert!(!glob_match("team.json", ".magistr/team.json"));
    assert!(glob_match("*team.json", ".magistr/team.json"));
    assert!(!glob_match("rm -rf", "sudo rm -rf /"));
    assert!(glob_match("*rm -rf*", "sudo rm -rf /"));
}

#[test]
fn consecutive_stars_do_not_blow_up() {
    assert!(glob_match("**", "abc"));
    assert!(glob_match("a**c", "abbbc"));
    assert!(!glob_match("a**d", "abbbc"));
}

#[test]
fn trailing_stars_after_the_text_still_match() {
    assert!(glob_match("abc*", "abc"));
    assert!(glob_match("abc***", "abc"));
    assert!(!glob_match("abc?", "abc"));
}

#[test]
fn backtracking_terminates_on_the_pathological_case() {
    assert!(!glob_match("*a*a*a*a*b", "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"));
}
