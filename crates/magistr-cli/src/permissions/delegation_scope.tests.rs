use super::*;

use magistr_engine::policy::PolicyRequest;

fn holds(scope: &DelegationScope, request: &PolicyRequest) -> bool {
    scope.condition.matches(request)
}

#[cfg(unix)]
#[test]
fn a_path_scopes_to_the_tool_over_its_first_two_directories() {
    let root = Path::new("/repo");
    let scope = DelegationScope::of(
        "Edit",
        &serde_json::json!({"file_path": "/repo/crates/magistr-desktop/src/view.rs"}),
        Some(root),
    )
    .expect("a scope");
    assert_eq!(scope.key, "edit:crates/magistr-desktop");
    assert_eq!(scope.subject, "Edit \u{b7} crates/magistr-desktop");
    // The relative spelling lands in the same scope.
    let relative = DelegationScope::of(
        "Edit",
        &serde_json::json!({"file_path": "crates/magistr-desktop/src/view.rs"}),
        Some(root),
    )
    .expect("a scope");
    assert_eq!(relative, scope);

    // The condition holds for both spellings of a path inside, for the
    // crate's own top-level file, and for nothing outside.
    for inside in [
        "/repo/crates/magistr-desktop/src/view.rs",
        "crates/magistr-desktop/src/view.rs",
        "crates/magistr-desktop/Cargo.toml",
    ] {
        assert!(
            holds(&scope, &PolicyRequest::tool("Edit").with_path(inside)),
            "{inside} is under the scope"
        );
    }
    assert!(!holds(
        &scope,
        &PolicyRequest::tool("Edit").with_path("crates/magistr-desktop-old/src/x.rs")
    ));
    assert!(!holds(
        &scope,
        &PolicyRequest::tool("Edit").with_path("crates/magistr-cockpit/src/view.rs")
    ));
    assert!(
        !holds(
            &scope,
            &PolicyRequest::tool("Write").with_path("crates/magistr-desktop/src/view.rs")
        ),
        "a scope is a tool over a place, never the place alone"
    );
}

#[cfg(unix)]
#[test]
fn a_shallow_path_scopes_to_what_it_has() {
    let one = DelegationScope::of("Write", &serde_json::json!({"path": "src/lib.rs"}), None)
        .expect("a scope");
    assert_eq!(one.key, "write:src");
    let file = DelegationScope::of(
        "Write",
        &serde_json::json!({"path": "Cargo.toml"}),
        Some(Path::new("/repo")),
    )
    .expect("a scope");
    assert_eq!(file.subject, "Write \u{b7} Cargo.toml");
    assert!(holds(
        &file,
        &PolicyRequest::tool("Write").with_path("Cargo.toml")
    ));
    assert!(holds(
        &file,
        &PolicyRequest::tool("Write").with_path("/repo/Cargo.toml")
    ));
    for elsewhere in ["crates/x/Cargo.toml", "/other/Cargo.toml"] {
        assert!(
            !holds(&file, &PolicyRequest::tool("Write").with_path(elsewhere)),
            "{elsewhere} is another file"
        );
    }
    // With no root the absolute spelling has to stay open.
    let unrooted = DelegationScope::of("Write", &serde_json::json!({"path": "Cargo.toml"}), None)
        .expect("a scope");
    assert!(holds(
        &unrooted,
        &PolicyRequest::tool("Write").with_path("/anywhere/Cargo.toml")
    ));
}

#[cfg(unix)]
#[test]
fn an_absolute_path_outside_the_root_falls_back_to_the_tool() {
    let scope = DelegationScope::of(
        "Read",
        &serde_json::json!({"file_path": "/etc/hosts"}),
        Some(Path::new("/repo")),
    )
    .expect("a scope");
    assert_eq!(scope.key, "tool:read");
    assert_eq!(scope.condition, PolicyCondition::tool("Read"));
}

#[test]
fn a_command_scopes_to_its_program_past_the_cd_prefix() {
    let scope = DelegationScope::of(
        "Bash",
        &serde_json::json!({"command": "cd crates/x && cargo nextest run"}),
        None,
    )
    .expect("a scope");
    assert_eq!(scope.key, "bash:cargo");
    assert_eq!(scope.subject, "Bash \u{b7} cargo");
    for command in [
        "cargo",
        "cargo test",
        "cd /repo && cargo build",
        "cd x && cargo",
    ] {
        assert!(
            holds(&scope, &PolicyRequest::tool("Bash").with_command(command)),
            "{command} runs the program"
        );
    }
    assert!(!holds(
        &scope,
        &PolicyRequest::tool("Bash").with_command("cargo-x run")
    ));
    assert!(!holds(
        &scope,
        &PolicyRequest::tool("Bash").with_command("rm -rf x && cargo test")
    ));
}

#[test]
fn a_glob_shaped_program_or_an_empty_tool_is_no_scope() {
    assert_eq!(
        DelegationScope::of("Bash", &serde_json::json!({"command": "* x"}), None).map(|s| s.key),
        Some("tool:bash".to_owned()),
        "a program that would widen to everything is not a program"
    );
    assert_eq!(
        DelegationScope::of("  ", &serde_json::json!({}), None),
        None
    );
}
