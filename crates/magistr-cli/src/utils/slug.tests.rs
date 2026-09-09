use super::*;

#[test]
fn basic_spaces() {
    assert_eq!(sanitize_prefix("My Project"), "my-project");
}

#[test]
fn special_chars_scoped_package() {
    assert_eq!(sanitize_prefix("@scope/my-pkg"), "scope-my-pkg");
}

#[test]
fn already_clean() {
    assert_eq!(sanitize_prefix("magistr"), "magistr");
}

#[test]
fn underscores_become_dashes() {
    assert_eq!(sanitize_prefix("hello_world"), "hello-world");
}

#[test]
fn preserves_existing_dashes() {
    assert_eq!(sanitize_prefix("already-clean"), "already-clean");
}

#[test]
fn consecutive_special_chars_collapsed() {
    assert_eq!(sanitize_prefix("a///b"), "a-b");
    assert_eq!(sanitize_prefix("a---b"), "a-b");
    assert_eq!(sanitize_prefix("a___b"), "a-b");
}

#[test]
fn leading_special_chars_trimmed() {
    assert_eq!(sanitize_prefix("@leading"), "leading");
    assert_eq!(sanitize_prefix("///leading"), "leading");
}

#[test]
fn trailing_special_chars_trimmed() {
    assert_eq!(sanitize_prefix("trailing!!!"), "trailing");
    assert_eq!(sanitize_prefix("trailing---"), "trailing");
}

#[test]
fn mixed_case_lowered() {
    assert_eq!(sanitize_prefix("MyApp"), "myapp");
    assert_eq!(sanitize_prefix("My-App"), "my-app");
}

#[test]
fn empty_input() {
    assert_eq!(sanitize_prefix(""), "");
}

#[test]
fn only_special_chars() {
    assert_eq!(sanitize_prefix("@#$%"), "");
}

#[test]
fn unicode_replaced_with_dashes() {
    // Non-ASCII chars (é) become dashes, trailing dashes trimmed
    assert_eq!(sanitize_prefix("café"), "caf");
    // Unicode between ASCII letters: "naïve" → "na-ve"
    assert_eq!(sanitize_prefix("naïve"), "na-ve");
}

#[test]
fn numbers_preserved() {
    assert_eq!(sanitize_prefix("project123"), "project123");
    assert_eq!(sanitize_prefix("v2.0"), "v2-0");
}

#[test]
fn complex_real_world_names() {
    assert_eq!(
        sanitize_prefix("My Cool Project (v2)"),
        "my-cool-project-v2"
    );
    assert_eq!(sanitize_prefix("@org/pkg-name_v2"), "org-pkg-name-v2");
}
