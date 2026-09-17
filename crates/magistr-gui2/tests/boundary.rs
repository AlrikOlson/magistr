//! The window declares what it can show; it never impersonates a terminal. No file
//! under `crates/magistr-gui2` may reach magistr-tui — the manifest has no such
//! dependency, and the source must not spell the path either — and none may fold a
//! terminal protocol into `Msg::Mode`: the pixels this surface has are declared through
//! `magistr_cockpit::capability::Mode`, which has no protocol to name.

use std::path::{Path, PathBuf};

fn crate_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if path.file_name().is_some_and(|n| n == "target") {
                continue;
            }
            walk(&path, out);
        } else if path.extension().is_some_and(|e| e == "rs" || e == "toml") {
            out.push(path);
        }
    }
}

/// Spelled so this file does not itself trip the check.
fn forbidden() -> Vec<String> {
    [
        ["magistr", "_tui"],
        ["magistr", "-tui = "],
        ["Protocol::", "Kitty"],
        ["Protocol::", "Sixel"],
        ["Passthrough", "::"],
        ["Mode::Graphics", "("],
    ]
    .into_iter()
    .map(|pair| pair.concat())
    .collect()
}

#[test]
fn the_window_never_names_the_terminal_crate_or_a_terminal_protocol() {
    let mut files = Vec::new();
    walk(&crate_root(), &mut files);
    assert!(files.len() >= 40, "walked only {} files", files.len());
    let this_file = Path::new(file!()).file_name().map(|n| n.to_os_string());
    let needles = forbidden();
    let mut offenders = Vec::new();
    for file in files {
        if file.file_name().map(|n| n.to_os_string()) == this_file {
            continue;
        }
        let source = std::fs::read_to_string(&file).expect("read source");
        for (idx, line) in source.lines().enumerate() {
            for needle in &needles {
                if line.contains(needle.as_str()) {
                    offenders.push(format!("{}:{}: {needle}", file.display(), idx + 1));
                }
            }
        }
    }
    assert!(
        offenders.is_empty(),
        "the window impersonates a terminal:\n{}",
        offenders.join("\n")
    );
}

#[test]
fn the_frame_never_dispatches_application_messages() {
    let source = std::fs::read_to_string(crate_root().join("src/frame.rs")).expect("frame source");
    for call in [
        "update(",
        "route.fold(",
        "commands.send(",
        "std::fs::",
        "std::process::",
    ] {
        assert!(
            !source.contains(call),
            "painting must use layout reports only: {call}"
        );
    }
}

#[test]
fn the_desktop_build_is_independent_of_terminal_and_store_implementations() {
    let root = crate_root();
    let version = std::process::Command::new("rustc")
        .arg("-vV")
        .output()
        .expect("rustc host");
    assert!(version.status.success());
    let version = String::from_utf8(version.stdout).expect("rustc version text");
    let host = version
        .lines()
        .find_map(|line| line.strip_prefix("host: "))
        .expect("host triple");
    let metadata = std::process::Command::new("cargo")
        .args([
            "metadata",
            "--filter-platform",
            host,
            "--offline",
            "--locked",
            "--format-version",
            "1",
            "--manifest-path",
        ])
        .arg(root.join("Cargo.toml"))
        .output()
        .expect("cargo metadata");
    assert!(
        metadata.status.success(),
        "{}",
        String::from_utf8_lossy(&metadata.stderr)
    );
    let metadata: serde_json::Value =
        serde_json::from_slice(&metadata.stdout).expect("metadata JSON");
    let packages = metadata["packages"].as_array().expect("packages");
    for forbidden in [
        "magistr-tui",
        "magistr-engine",
        "magistr-workflow",
        "magistr-workspace",
        "magistr-process",
        "magistr-acp",
        "magistr-cli",
    ] {
        assert!(
            !packages.iter().any(|p| p["name"] == forbidden),
            "desktop reaches {forbidden}"
        );
    }
    assert_eq!(
        metadata["workspace_members"]
            .as_array()
            .expect("members")
            .len(),
        1
    );
    let public = std::process::Command::new("cargo")
        .args([
            "metadata",
            "--filter-platform",
            host,
            "--offline",
            "--locked",
            "--no-deps",
            "--format-version",
            "1",
            "--manifest-path",
        ])
        .arg(root.join("../../Cargo.toml"))
        .output()
        .expect("public metadata");
    assert!(
        public.status.success(),
        "{}",
        String::from_utf8_lossy(&public.stderr)
    );
    let public: serde_json::Value = serde_json::from_slice(&public.stdout).expect("public JSON");
    assert!(
        !public["packages"]
            .as_array()
            .expect("public packages")
            .iter()
            .any(|p| p["name"] == "magistr-gui2")
    );
}
