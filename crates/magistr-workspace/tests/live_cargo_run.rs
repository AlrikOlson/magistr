//! The retained goldens' staleness guard (specs/013 SC-006): the real cargo
//! toolchain over a fixture workspace, executed through the production
//! [`ProcessExecutor`] under the declared-command use case, parsed by the
//! same runner and diagnostics parsers the goldens pin. Skips honestly when
//! cargo is not on PATH; a network-free fixture (no dependencies) keeps it
//! offline and short.

use std::{path::Path, process::Command};

use magistr_workspace::{
    config::ResolveOptions,
    contracts::WorkspaceResponse,
    execution::{ProcessExecutor, RunInput, RunRow, WorkspaceRun},
    workspace_policy::WorkspacePolicy,
};
use tokio_util::sync::CancellationToken;

fn cargo_available() -> bool {
    Command::new("cargo")
        .arg("--version")
        .output()
        .is_ok_and(|output| output.status.success())
}

fn write(root: &Path, relative: &str, content: &str) {
    let path = root.join(relative);
    std::fs::create_dir_all(path.parent().expect("parent")).expect("fixture dir");
    std::fs::write(path, content).expect("fixture file");
}

fn fixture_workspace(root: &Path) {
    write(
        root,
        "Cargo.toml",
        "[workspace]\nresolver = \"2\"\nmembers = [\"crates/*\"]\n",
    );
    write(
        root,
        "crates/alpha/Cargo.toml",
        "[package]\nname = \"alpha\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
    );
    write(
        root,
        "crates/alpha/src/lib.rs",
        concat!(
            "pub fn add(a: i32, b: i32) -> i32 { a + b }\n",
            "pub fn unused_thing(x: i32) -> i32 { let y = x; x }\n",
            "#[cfg(test)]\n",
            "mod tests {\n",
            "    use super::*;\n",
            "    #[test]\n",
            "    fn adds() { assert_eq!(add(1, 2), 3); }\n",
            "    #[test]\n",
            "    fn fails_panic() { panic!(\"boom {}\", 42); }\n",
            "}\n",
        ),
    );
    write(
        root,
        "crates/beta/Cargo.toml",
        "[package]\nname = \"beta\"\nversion = \"0.1.0\"\nedition = \"2021\"\n[dependencies]\nalpha = { path = \"../alpha\" }\n",
    );
    write(
        root,
        "crates/beta/src/lib.rs",
        "pub fn twice(a: i32) -> i32 { alpha::add(a, a) }\n",
    );
    write(
        root,
        "crates/beta/tests/integration.rs",
        "#[test]\nfn integration_fails() { let v: Vec<i32> = vec![]; let _ = v[0]; }\n",
    );
    write(
        root,
        ".magistr/workspace.toml",
        concat!(
            "[commands]\n",
            "test = \"cargo test --workspace --no-fail-fast\"\n",
            "build-json = \"cargo build --workspace --message-format=json\"\n",
            "build = \"cargo build --workspace\"\n",
        ),
    );
}

fn row(response: WorkspaceResponse<RunRow>) -> RunRow {
    match response {
        WorkspaceResponse::Success { results, .. } => results.rows()[0].clone(),
        WorkspaceResponse::Failure { error, .. } => panic!("expected a row, got {error}"),
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn real_cargo_over_a_fixture_workspace_matches_the_goldens_shape() {
    if !cargo_available() {
        eprintln!("cargo not available; live toolchain check skipped");
        return;
    }
    let dir = tempfile::tempdir().expect("fixture root");
    fixture_workspace(dir.path());
    let policy = WorkspacePolicy::resolve(&ResolveOptions::new(dir.path())).expect("policy");
    let executor = ProcessExecutor::new(dir.path());
    let run = WorkspaceRun::new(&policy, &executor);
    let input = |command: &str| RunInput::new(command).with_timeout_secs(600);

    let RunRow::Report {
        adapter,
        exit_code,
        failures,
        summary,
        ..
    } = row(run.run(&input("test"), CancellationToken::new()).await)
    else {
        panic!("cargo test answers a report row");
    };
    assert_eq!(adapter, "cargo");
    assert_eq!(exit_code, Some(101));
    let names: Vec<&str> = failures
        .iter()
        .map(|failure| failure.test_name.as_str())
        .collect();
    assert!(names.contains(&"alpha tests::fails_panic"), "{names:?}");
    assert!(
        names.contains(&"integration integration_fails"),
        "{names:?}"
    );
    let panic = failures
        .iter()
        .find(|failure| failure.test_name == "alpha tests::fails_panic")
        .expect("panic failure");
    assert_eq!(panic.assertion, "boom 42");
    assert_eq!(panic.file.as_deref(), Some("crates/alpha/src/lib.rs"));
    let summary = summary.expect("summary");
    assert!(summary.contains("2 failed"), "{summary}");
    assert!(
        summary.contains("rerun: -p alpha --lib, -p beta --test integration"),
        "{summary}"
    );

    // A compile error joins the workspace: JSON and rendered agree.
    write(
        dir.path(),
        "crates/gamma/Cargo.toml",
        "[package]\nname = \"gamma\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
    );
    write(
        dir.path(),
        "crates/gamma/src/lib.rs",
        "pub fn broken() -> i32 { let s: String = 1; s }\n",
    );
    let RunRow::Diagnostics {
        diagnostics: json_diagnostics,
        exit_code,
        summary: json_summary,
        ..
    } = row(run
        .run(&input("build-json"), CancellationToken::new())
        .await)
    else {
        panic!("a failed build answers diagnostics");
    };
    assert_eq!(exit_code, Some(101));
    assert_eq!(json_diagnostics[0].code.as_deref(), Some("E0308"));
    assert_eq!(
        json_diagnostics[0].file.as_deref(),
        Some("crates/gamma/src/lib.rs")
    );
    assert!(
        json_diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code.as_deref() == Some("unused_variables"))
    );
    let RunRow::Diagnostics {
        diagnostics: rendered_diagnostics,
        summary: rendered_summary,
        ..
    } = row(run.run(&input("build"), CancellationToken::new()).await)
    else {
        panic!("the rendered build answers diagnostics");
    };
    assert_eq!(rendered_diagnostics, json_diagnostics);
    assert_eq!(rendered_summary, json_summary);
}
