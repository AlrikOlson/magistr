//! The executable dispatches protocol children without opening a native window.

#[test]
fn protocol_roles_use_the_actual_desktop_executable() {
    let result = std::process::Command::new(if cfg!(windows) { "python" } else { "python3" })
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
    assert!(graph.contains("tauri "), "{graph}");
    for retired in [
        "magistr-tui ",
        "magistr-gui2 ",
        "qs-core ",
        "qs-render ",
        "qs-widgets ",
    ] {
        assert!(!graph.contains(retired), "{graph}");
    }
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    for retired in ["magistr-tui", "magistr-gui2"] {
        assert!(
            !root.join("crates").join(retired).exists(),
            "{retired} remains"
        );
    }
}
