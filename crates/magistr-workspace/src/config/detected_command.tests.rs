use super::*;

#[test]
fn lockfile_selects_node_runner_and_detection_never_writes() {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(
        root.path().join("package.json"),
        r#"{"scripts":{"test":"vitest","custom":"x","lint":false}}"#,
    )
    .unwrap();
    std::fs::write(root.path().join("pnpm-lock.yaml"), "").unwrap();
    std::fs::write(root.path().join("bun.lock"), "").unwrap();
    let commands = DetectedCommand::detect(root.path());
    assert_eq!(commands.len(), 1);
    assert_eq!(commands[0].line, "bun run test");
    assert!(!root.path().join(".magistr/workspace.toml").exists());
    assert_eq!(std::fs::read_dir(root.path()).unwrap().count(), 3);
}

#[test]
fn cargo_workspace_expands_members_and_declared_tools_only() {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("Cargo.toml"), "[workspace]\nmembers=['crates/*','unknown/**']\nexclude=['crates/skip']\n[package]\nname='root'\n").unwrap();
    for (dir, name) in [("a", "zeta"), ("b", "alpha"), ("skip", "skip")] {
        let member = root.path().join("crates").join(dir);
        std::fs::create_dir_all(&member).unwrap();
        std::fs::write(
            member.join("Cargo.toml"),
            format!("[package]\nname='{name}'\n"),
        )
        .unwrap();
    }
    let commands = DetectedCommand::detect(root.path());
    assert_eq!(commands[0].line, "cargo test --workspace --no-fail-fast");
    assert!(commands[0].provenance.contains("unknown/**"));
    assert_eq!(
        commands
            .iter()
            .skip(2)
            .map(|c| c.name.as_str())
            .collect::<Vec<_>>(),
        ["cargo-test-alpha", "cargo-test-root", "cargo-test-zeta"]
    );
    assert!(
        !commands
            .iter()
            .any(|c| c.name == "cargo-nextest" || c.name == "cargo-clippy")
    );
    std::fs::create_dir(root.path().join(".config")).unwrap();
    std::fs::write(root.path().join(".config/nextest.toml"), "").unwrap();
    std::fs::write(
        root.path().join("rust-toolchain.toml"),
        "[toolchain]\ncomponents=['clippy']\n",
    )
    .unwrap();
    let commands = DetectedCommand::detect(root.path());
    assert!(
        commands
            .iter()
            .any(|c| c.line == "cargo nextest run --workspace --no-fail-fast")
    );
    assert!(
        commands
            .iter()
            .any(|c| c.line == "cargo clippy --workspace --all-targets")
    );
}

#[test]
fn go_pytest_and_single_crate_suggestions_are_stable() {
    let root = tempfile::tempdir().unwrap();
    for file in ["Cargo.toml", "go.mod", "pytest.ini"] {
        std::fs::write(root.path().join(file), "").unwrap();
    }
    let commands = DetectedCommand::detect(root.path());
    assert_eq!(
        commands.iter().map(|c| c.line.as_str()).collect::<Vec<_>>(),
        [
            "cargo test",
            "cargo build",
            "go test ./...",
            "go build ./...",
            "pytest"
        ]
    );
}
