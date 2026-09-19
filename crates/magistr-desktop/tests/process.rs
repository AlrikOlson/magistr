//! The executable dispatches protocol children without opening a native window.

#[test]
fn protocol_roles_use_the_actual_desktop_executable() {
    let result = std::process::Command::new("python3")
        .arg(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/process_roles.py"
        ))
        .arg(env!("CARGO_BIN_EXE_magistr-desktop"))
        .output()
        .expect("Python process contract driver");
    assert!(
        result.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );
}

#[test]
fn live_desktop_has_the_host_and_no_terminal_renderer() {
    let result = std::process::Command::new("cargo")
        .args([
            "tree",
            "--offline",
            "--locked",
            "--edges",
            "normal",
            "--prefix",
            "none",
            "--manifest-path",
        ])
        .arg(concat!(env!("CARGO_MANIFEST_DIR"), "/Cargo.toml"))
        .output()
        .expect("resolved desktop graph");
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let graph = String::from_utf8(result.stdout).unwrap();
    assert!(graph.contains("magistr-cli "));
    assert!(graph.contains("magistr-gui2 "));
    assert!(!graph.contains("magistr-tui "), "{graph}");
}
