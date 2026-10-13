//! The host's current resolved graph contains no retired presentation runtime.
#[test]
fn the_host_library_has_no_terminal_or_native_renderer_dependency() {
    let output = std::process::Command::new("cargo")
        .args([
            "tree",
            "--locked",
            "--offline",
            "-p",
            "magistr-cli",
            "--edges",
            "normal",
            "--prefix",
            "none",
        ])
        .output()
        .expect("Cargo graph");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let graph = String::from_utf8(output.stdout).unwrap();
    assert!(graph.contains("magistr-cockpit-runtime "));
    for retired in ["magistr-tui ", "magistr-gui2 ", "qs-core ", "tauri "] {
        assert!(!graph.contains(retired), "{graph}");
    }
}
