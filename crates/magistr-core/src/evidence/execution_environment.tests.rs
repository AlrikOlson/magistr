use super::*;

fn variables(pairs: &[(&str, &str)]) -> Vec<(OsString, OsString)> {
    pairs
        .iter()
        .map(|(name, value)| (OsString::from(name), OsString::from(value)))
        .collect()
}

fn names(bound: &[(String, Vec<u8>)]) -> Vec<&str> {
    bound.iter().map(|(name, _)| name.as_str()).collect()
}

const CARGO: &str = "rustc cargo cargo-nextest";

#[test]
fn a_new_session_of_the_host_binds_the_same_environment() {
    let tools = [("PATH", "/usr/bin"), ("CARGO_TARGET_DIR", "target")];
    let mut first = variables(&tools);
    first.extend(variables(&[
        ("CLAUDE_CODE_SESSION_ID", "one"),
        ("CLAUDE_PID", "4120"),
        ("WT_SESSION", "a1"),
        ("API_TOKEN", "secret-one"),
        ("TEMP", "/tmp/one"),
    ]));
    let mut second = variables(&tools);
    second.extend(variables(&[
        ("CLAUDE_CODE_SESSION_ID", "two"),
        ("CLAUDE_PID", "9004"),
        ("WT_SESSION", "b2"),
        ("API_TOKEN", "secret-two"),
        ("TEMP", "/tmp/two"),
    ]));
    assert_eq!(bound(first, CARGO, &[]), bound(second, CARGO, &[]));
}

#[test]
fn what_chooses_or_steers_a_tool_is_bound_with_its_value() {
    let before = variables(&[("PATH", "/usr/bin"), ("RUSTFLAGS", "-D warnings")]);
    let moved = variables(&[("PATH", "/opt/bin"), ("RUSTFLAGS", "-D warnings")]);
    let flagged = variables(&[("PATH", "/usr/bin"), ("RUSTFLAGS", "")]);
    assert_ne!(bound(before.clone(), CARGO, &[]), bound(moved, CARGO, &[]));
    assert_ne!(bound(before, CARGO, &[]), bound(flagged, CARGO, &[]));
}

#[test]
fn a_tools_variables_are_bound_only_for_a_command_that_uses_it() {
    let all = variables(&[
        ("CARGO_INCREMENTAL", "0"),
        ("RUSTC_WRAPPER", "sccache"),
        ("RUSTUP_TOOLCHAIN", "stable"),
        ("RUST_TEST_THREADS", "1"),
        ("NEXTEST_PROFILE", "ci"),
        ("NODE_OPTIONS", "--max-old-space-size=4096"),
        ("npm_config_registry", "https://registry.example"),
        ("PYTHONPATH", "src"),
        ("VIRTUAL_ENV", ".venv"),
        ("GIT_DIR", ".git"),
        ("RustRover", "C:/Program Files/RustRover"),
    ]);
    assert_eq!(
        names(&bound(all.clone(), CARGO, &[])),
        [
            "CARGO_INCREMENTAL",
            "NEXTEST_PROFILE",
            "RUSTC_WRAPPER",
            "RUSTUP_TOOLCHAIN",
            "RUST_TEST_THREADS"
        ]
    );
    assert_eq!(
        names(&bound(all.clone(), "node npm", &[])),
        ["NODE_OPTIONS", "NPM_CONFIG_REGISTRY"]
    );
    assert_eq!(
        names(&bound(all.clone(), "python python3", &[])),
        ["PYTHONPATH", "VIRTUAL_ENV"]
    );
    assert_eq!(names(&bound(all.clone(), "git", &[])), ["GIT_DIR"]);
    assert!(bound(all, "", &[]).is_empty());
}

#[test]
fn a_name_is_matched_whatever_its_case() {
    let windows = variables(&[("Path", "C:/Windows"), ("PathExt", ".EXE")]);
    assert_eq!(names(&bound(windows, "", &[])), ["PATH", "PATHEXT"]);
}

#[test]
fn a_project_declares_the_variables_its_own_checks_read() {
    let all = variables(&[
        ("DATABASE_URL", "postgres://one"),
        ("APP_REGION", "eu"),
        ("APP_TIER", "high"),
        ("APPDATA", "C:/Users/a/AppData"),
        ("OTHER", "x"),
    ]);
    assert!(bound(all.clone(), "", &[]).is_empty());
    let declared = ["DATABASE_URL".to_owned(), "app_*".to_owned()];
    assert_eq!(
        names(&bound(all.clone(), "", &declared)),
        ["APP_REGION", "APP_TIER", "DATABASE_URL"]
    );
    assert_eq!(bound(all, "", &["*".to_owned()]).len(), 5);
}
